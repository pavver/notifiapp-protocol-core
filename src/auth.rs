use crate::codec::ProtocolCodec;
use async_trait::async_trait;
use notifiapp_transport::{AuthHandler, AuthOutcome};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Outcome of parsing a custom authentication response.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedAuthResponse {
    /// Authentication succeeded and returned a new session resumption token.
    Success { token: String },
    /// Session has expired on the server (resume token no longer valid).
    SessionExpired,
    /// Authentication credentials (username/password) were rejected by the server.
    Unauthorized,
    /// Unrecognized response format or parsing error.
    InvalidResponse,
}

/// Events emitted by the session authentication handler when connection is established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSessionEvent {
    /// Session was successfully resumed using a token (existing server subscriptions are kept).
    SessionResumed,
    /// A new session was created using login/credentials (existing server subscriptions are reset).
    NewSessionCreated,
    /// Session restoration failed (token was invalid/expired).
    SessionRestoreFailed,
}

/// Generic session-based authentication handler.
///
/// Implements `AuthHandler` from `notifiapp-transport` and automates:
/// 1. Resuming sessions via a saved token (`Resume`).
/// 2. Logging in via username/password (`Login`) if no token exists or if the token has expired.
/// 3. Saving the new resumption token on success.
/// 4. Resetting the session state on expiration.
pub struct SessionAuthHandler<Action, Response, Codec> {
    /// Username and password credentials.
    credentials: Arc<RwLock<Option<(String, String)>>>,
    /// Session resumption token.
    token: Arc<RwLock<Option<String>>>,
    /// Builder to create an outgoing auth Action using username and password.
    login_builder: Arc<dyn Fn(String, String) -> Action + Send + Sync>,
    /// Builder to create an outgoing auth Action using a resumption token.
    resume_builder: Arc<dyn Fn(String) -> Action + Send + Sync>,
    /// Parser to extract auth outcome from the deserialized response message.
    response_parser: Arc<dyn Fn(Response) -> ParsedAuthResponse + Send + Sync>,
    /// Publisher for session events.
    event_tx: tokio::sync::broadcast::Sender<AuthSessionEvent>,
    /// Phantom data to satisfy the compiler for the codec type.
    _phantom: std::marker::PhantomData<Codec>,
}

impl<Action, Response, Codec> SessionAuthHandler<Action, Response, Codec>
where
    Action: serde::Serialize + Send + Sync + 'static,
    Response: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Codec: ProtocolCodec,
{
    /// Create a new session authentication handler.
    pub fn new(
        login_builder: impl Fn(String, String) -> Action + Send + Sync + 'static,
        resume_builder: impl Fn(String) -> Action + Send + Sync + 'static,
        response_parser: impl Fn(Response) -> ParsedAuthResponse + Send + Sync + 'static,
    ) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(16);
        Self {
            credentials: Arc::new(RwLock::new(None)),
            token: Arc::new(RwLock::new(None)),
            login_builder: Arc::new(login_builder),
            resume_builder: Arc::new(resume_builder),
            response_parser: Arc::new(response_parser),
            event_tx,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Subscribe to session auth events.
    pub fn subscribe_session_events(&self) -> tokio::sync::broadcast::Receiver<AuthSessionEvent> {
        self.event_tx.subscribe()
    }

    /// Update user credentials (typically triggered by a login call).
    pub async fn set_credentials(&self, user: String, pass: String) {
        let mut creds = self.credentials.write().await;
        *creds = Some((user, pass));
        let mut tok = self.token.write().await;
        *tok = None; // Reset token on new login
    }

    /// Retrieve the current resume token if available.
    pub async fn get_token(&self) -> Option<String> {
        self.token.read().await.clone()
    }

    /// Update the current resume token.
    pub async fn set_token(&self, token: String) {
        *self.token.write().await = Some(token);
    }

    /// Reset all session state (credentials and token).
    pub async fn logout(&self) {
        *self.credentials.write().await = None;
        *self.token.write().await = None;
    }
}

#[async_trait]
impl<Action, Response, Codec> AuthHandler for SessionAuthHandler<Action, Response, Codec>
where
    Action: serde::Serialize + Send + Sync + 'static,
    Response: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Codec: ProtocolCodec,
{
    async fn auth_payload(&self) -> Option<Vec<u8>> {
        // 1. Try to resume first if we have a token
        if let Some(token) = self.get_token().await {
            let action = (self.resume_builder)(token);
            if let Ok(bytes) = Codec::serialize(&action) {
                return Some(bytes);
            }
        }

        // 2. Fallback to login if credentials are available
        if let Some((user, pass)) = self.credentials.read().await.clone() {
            let action = (self.login_builder)(user, pass);
            if let Ok(bytes) = Codec::serialize(&action) {
                return Some(bytes);
            }
        }

        None
    }

    async fn process_auth_response(&self, response_bytes: &[u8]) -> AuthOutcome {
        // Deserialise using the specified protocol codec
        let resp: Response = match Codec::deserialize(response_bytes) {
            Ok(r) => r,
            Err(_) => return AuthOutcome::Failed,
        };

        // Determine if we attempted resume or login by checking if token was set
        let attempted_resume = self.get_token().await.is_some();

        match (self.response_parser)(resp) {
            ParsedAuthResponse::Success { token } => {
                self.set_token(token).await;
                if attempted_resume {
                    let _ = self.event_tx.send(AuthSessionEvent::SessionResumed);
                } else {
                    let _ = self.event_tx.send(AuthSessionEvent::NewSessionCreated);
                }
                AuthOutcome::Success
            }
            ParsedAuthResponse::SessionExpired => {
                // Clear the expired resume token
                let mut tok = self.token.write().await;
                *tok = None;

                let _ = self.event_tx.send(AuthSessionEvent::SessionRestoreFailed);

                // Check if we can fallback to credentials immediately
                if attempted_resume && self.credentials.read().await.is_some() {
                    // Try to re-authenticate with credentials during the next reconnection round
                    AuthOutcome::Failed
                } else {
                    AuthOutcome::Unauthorized
                }
            }
            ParsedAuthResponse::Unauthorized => {
                // Server explicitly rejected credentials
                *self.credentials.write().await = None;
                *self.token.write().await = None;
                if attempted_resume {
                    let _ = self.event_tx.send(AuthSessionEvent::SessionRestoreFailed);
                }
                AuthOutcome::Unauthorized
            }
            ParsedAuthResponse::InvalidResponse => {
                if attempted_resume {
                    let _ = self.event_tx.send(AuthSessionEvent::SessionRestoreFailed);
                }
                AuthOutcome::Failed
            }
        }
    }

    async fn on_session_expired(&self) {
        // Session expired notification from server during normal operation
        *self.token.write().await = None;
        let _ = self.event_tx.send(AuthSessionEvent::SessionRestoreFailed);
    }
}
