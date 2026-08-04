use crate::codec::ProtocolCodec;
use async_trait::async_trait;
use notifiapp_transport::{AuthHandler, AuthOutcome};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Outcome of parsing a custom authentication response.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedAuthResponse {
    /// Authentication succeeded and optionally returned a new session resumption token.
    Success { token: Option<String> },
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

/// A protocol-agnostic trait for authentication strategies.
#[async_trait]
pub trait AuthProtocol<Action, Response>: Send + Sync + 'static {
    /// Return the payload to send for authentication.
    async fn get_auth_action(&self) -> Option<Action>;

    /// Process the server response and return AuthOutcome and optional AuthSessionEvent.
    async fn process_auth_response(
        &self,
        response: Response,
    ) -> (AuthOutcome, Option<AuthSessionEvent>);

    /// Called when the session expires.
    async fn on_session_expired(&self) -> Option<AuthSessionEvent>;
}

/// A wrapper that adapts a typed `AuthProtocol` to the transport's `AuthHandler`.
pub struct TypedAuthWrapper<Action, Response, Codec> {
    pub protocol: Arc<dyn AuthProtocol<Action, Response>>,
    event_tx: tokio::sync::broadcast::Sender<AuthSessionEvent>,
    _phantom: PhantomData<Codec>,
}

impl<Action, Response, Codec> TypedAuthWrapper<Action, Response, Codec>
where
    Action: serde::Serialize + Send + Sync + 'static,
    Response: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Codec: ProtocolCodec,
{
    pub fn new(protocol: Arc<dyn AuthProtocol<Action, Response>>) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(16);
        Self {
            protocol,
            event_tx,
            _phantom: PhantomData,
        }
    }

    pub fn subscribe_session_events(&self) -> tokio::sync::broadcast::Receiver<AuthSessionEvent> {
        self.event_tx.subscribe()
    }
}

#[async_trait]
impl<Action, Response, Codec> AuthHandler for TypedAuthWrapper<Action, Response, Codec>
where
    Action: serde::Serialize + Send + Sync + 'static,
    Response: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Codec: ProtocolCodec,
{
    async fn auth_payload(&self) -> Option<Vec<u8>> {
        if let Some(action) = self.protocol.get_auth_action().await {
            Codec::serialize(&action).ok()
        } else {
            None
        }
    }

    async fn process_auth_response(&self, response_bytes: &[u8]) -> AuthOutcome {
        let resp: Response = match Codec::deserialize(response_bytes) {
            Ok(r) => r,
            Err(_) => return AuthOutcome::Failed,
        };

        let (outcome, event) = self.protocol.process_auth_response(resp).await;
        if let Some(ev) = event {
            let _ = self.event_tx.send(ev);
        }
        outcome
    }

    async fn on_session_expired(&self) {
        if let Some(ev) = self.protocol.on_session_expired().await {
            let _ = self.event_tx.send(ev);
        }
    }
}

// ---------------------------------------------------------------------------
// Provided implementations
// ---------------------------------------------------------------------------

/// Standard login/password authentication with session token resumption.
pub struct CredentialsAuth<Action, Response> {
    credentials: Arc<RwLock<Option<(String, String)>>>,
    token: Arc<RwLock<Option<String>>>,
    login_builder: Arc<dyn Fn(String, String) -> Action + Send + Sync>,
    resume_builder: Arc<dyn Fn(String) -> Action + Send + Sync>,
    response_parser: Arc<dyn Fn(Response) -> ParsedAuthResponse + Send + Sync>,
}

impl<Action, Response> CredentialsAuth<Action, Response> {
    pub fn new(
        login_builder: impl Fn(String, String) -> Action + Send + Sync + 'static,
        resume_builder: impl Fn(String) -> Action + Send + Sync + 'static,
        response_parser: impl Fn(Response) -> ParsedAuthResponse + Send + Sync + 'static,
    ) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(None)),
            token: Arc::new(RwLock::new(None)),
            login_builder: Arc::new(login_builder),
            resume_builder: Arc::new(resume_builder),
            response_parser: Arc::new(response_parser),
        }
    }

    pub async fn set_credentials(&self, user: String, pass: String) {
        let mut creds = self.credentials.write().await;
        *creds = Some((user, pass));
        let mut tok = self.token.write().await;
        *tok = None;
    }

    pub async fn logout(&self) {
        *self.credentials.write().await = None;
        *self.token.write().await = None;
    }
}

#[async_trait]
impl<Action, Response> AuthProtocol<Action, Response> for CredentialsAuth<Action, Response>
where
    Action: Send + Sync + 'static,
    Response: Send + Sync + 'static,
{
    async fn get_auth_action(&self) -> Option<Action> {
        if let Some(token) = self.token.read().await.clone() {
            return Some((self.resume_builder)(token));
        }
        if let Some((user, pass)) = self.credentials.read().await.clone() {
            return Some((self.login_builder)(user, pass));
        }
        None
    }

    async fn process_auth_response(
        &self,
        response: Response,
    ) -> (AuthOutcome, Option<AuthSessionEvent>) {
        let attempted_resume = self.token.read().await.is_some();
        match (self.response_parser)(response) {
            ParsedAuthResponse::Success { token } => {
                if let Some(t) = token {
                    *self.token.write().await = Some(t);
                }
                let event = if attempted_resume {
                    AuthSessionEvent::SessionResumed
                } else {
                    AuthSessionEvent::NewSessionCreated
                };
                (AuthOutcome::Success, Some(event))
            }
            ParsedAuthResponse::SessionExpired => {
                *self.token.write().await = None;
                if attempted_resume && self.credentials.read().await.is_some() {
                    (
                        AuthOutcome::Failed,
                        Some(AuthSessionEvent::SessionRestoreFailed),
                    )
                } else {
                    (
                        AuthOutcome::Unauthorized,
                        Some(AuthSessionEvent::SessionRestoreFailed),
                    )
                }
            }
            ParsedAuthResponse::Unauthorized => {
                *self.credentials.write().await = None;
                *self.token.write().await = None;
                let event = if attempted_resume {
                    Some(AuthSessionEvent::SessionRestoreFailed)
                } else {
                    None
                };
                (AuthOutcome::Unauthorized, event)
            }
            ParsedAuthResponse::InvalidResponse => {
                let event = if attempted_resume {
                    Some(AuthSessionEvent::SessionRestoreFailed)
                } else {
                    None
                };
                (AuthOutcome::Failed, event)
            }
        }
    }

    async fn on_session_expired(&self) -> Option<AuthSessionEvent> {
        *self.token.write().await = None;
        Some(AuthSessionEvent::SessionRestoreFailed)
    }
}

/// Static token authentication (e.g. Bearer token).
pub struct StaticTokenAuth<Action, Response> {
    token: String,
    action_builder: Arc<dyn Fn(String) -> Action + Send + Sync>,
    response_parser: Arc<dyn Fn(Response) -> ParsedAuthResponse + Send + Sync>,
}

impl<Action, Response> StaticTokenAuth<Action, Response> {
    pub fn new(
        token: String,
        action_builder: impl Fn(String) -> Action + Send + Sync + 'static,
        response_parser: impl Fn(Response) -> ParsedAuthResponse + Send + Sync + 'static,
    ) -> Self {
        Self {
            token,
            action_builder: Arc::new(action_builder),
            response_parser: Arc::new(response_parser),
        }
    }
}

#[async_trait]
impl<Action, Response> AuthProtocol<Action, Response> for StaticTokenAuth<Action, Response>
where
    Action: Send + Sync + 'static,
    Response: Send + Sync + 'static,
{
    async fn get_auth_action(&self) -> Option<Action> {
        Some((self.action_builder)(self.token.clone()))
    }

    async fn process_auth_response(
        &self,
        response: Response,
    ) -> (AuthOutcome, Option<AuthSessionEvent>) {
        match (self.response_parser)(response) {
            ParsedAuthResponse::Success { .. } => (
                AuthOutcome::Success,
                Some(AuthSessionEvent::NewSessionCreated),
            ),
            ParsedAuthResponse::Unauthorized => (AuthOutcome::Unauthorized, None),
            _ => (AuthOutcome::Failed, None),
        }
    }

    async fn on_session_expired(&self) -> Option<AuthSessionEvent> {
        None
    }
}
