use async_trait::async_trait;
use dashmap::DashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Error conditions that can occur during session operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// Session was not found for the provided token or ID.
    #[error("Session not found")]
    NotFound,
    /// Session has expired due to inactivity exceeding the session TTL.
    #[error("Session expired")]
    Expired,
    /// Session token reuse conflict detected (replay attack or duplicate connection).
    #[error("Session token reuse conflict detected")]
    Conflict,
}

/// Configuration settings for session management and token lifecycle.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Duration of inactivity after which a disconnected session is considered expired.
    pub session_ttl: Duration,
    /// Duration to retain used/rotated tokens to detect replay attacks and conflicts.
    pub used_token_ttl: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            session_ttl: Duration::from_secs(15 * 60),    // 15 minutes
            used_token_ttl: Duration::from_secs(30 * 60), // 30 minutes
        }
    }
}

/// A generic session container holding authentication and custom session state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session<UserId = Uuid, Data = ()> {
    /// Unique identifier for the active session.
    pub session_id: Uuid,
    /// Identifier of the authenticated user or principal.
    pub user_id: UserId,
    /// Current single-use resumption token.
    pub current_token: String,
    /// Timestamp of the last activity on this session.
    pub last_seen: Instant,
    /// Custom application-specific session data (e.g. subscriptions, roles, permissions).
    pub data: Data,
}

impl<UserId, Data> Deref for Session<UserId, Data> {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// An asynchronous abstraction for session persistence and lifecycle management.
#[async_trait]
pub trait SessionStore<UserId, Data>: Send + Sync + 'static {
    /// Create a new session for the given user and session data.
    async fn create_session(&self, user_id: UserId, data: Data) -> Arc<Session<UserId, Data>>;

    /// Resume an existing session using a resumption token, rotating to a new token.
    async fn resume_session(&self, token: &str)
    -> Result<Arc<Session<UserId, Data>>, SessionError>;

    /// Validate a token and touch `last_seen` without rotating the token.
    async fn validate_and_touch(
        &self,
        token: &str,
    ) -> Result<Arc<Session<UserId, Data>>, SessionError>;

    /// Retrieve a session by its active token without modifying `last_seen`.
    async fn get_session_by_token(&self, token: &str) -> Option<Arc<Session<UserId, Data>>>;

    /// Retrieve a session by its unique session ID.
    async fn get_session_by_id(&self, session_id: &Uuid) -> Option<Arc<Session<UserId, Data>>>;

    /// Update the last seen timestamp for a session.
    async fn update_last_seen(&self, session_id: &Uuid);

    /// Terminate and remove a specific session.
    async fn terminate_session(&self, session_id: &Uuid);

    /// Terminate all active sessions belonging to the specified user.
    async fn terminate_all_for_user(&self, user_id: &UserId);

    /// Perform maintenance cleanup of expired sessions and old token history.
    async fn cleanup(&self);
}

/// An in-memory, thread-safe session manager backed by `DashMap`.
pub struct SessionManager<UserId = Uuid, Data = ()> {
    /// Map of active session tokens to sessions.
    pub sessions_by_token: DashMap<String, Arc<Session<UserId, Data>>>,
    /// Map of session IDs to sessions.
    pub sessions_by_id: DashMap<Uuid, Arc<Session<UserId, Data>>>,
    /// Map of used (rotated) tokens to (session_id, user_id, rotated_at).
    pub used_tokens: DashMap<String, (Uuid, UserId, Instant)>,
    /// Configuration for TTL and token retention.
    pub config: SessionConfig,
}

impl<UserId, Data> Default for SessionManager<UserId, Data> {
    fn default() -> Self {
        Self::new()
    }
}

impl<UserId, Data> SessionManager<UserId, Data> {
    /// Create a new session manager with default configuration.
    pub fn new() -> Self {
        Self::with_config(SessionConfig::default())
    }

    /// Create a new session manager with custom configuration.
    pub fn with_config(config: SessionConfig) -> Self {
        Self {
            sessions_by_token: DashMap::new(),
            sessions_by_id: DashMap::new(),
            used_tokens: DashMap::new(),
            config,
        }
    }

    /// Return a reference to the session manager's configuration.
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Return the count of active sessions.
    pub fn active_session_count(&self) -> usize {
        self.sessions_by_id.len()
    }

    /// Create a new session with custom session data.
    pub fn create_session(&self, user_id: UserId, data: Data) -> Arc<Session<UserId, Data>> {
        let session_id = Uuid::new_v4();
        let token = Uuid::new_v4().to_string();

        let session = Arc::new(Session {
            session_id,
            user_id,
            current_token: token.clone(),
            last_seen: Instant::now(),
            data,
        });

        self.sessions_by_token.insert(token, Arc::clone(&session));
        self.sessions_by_id.insert(session_id, Arc::clone(&session));
        session
    }

    /// Resume an existing session, validating TTL and rotating the token.
    pub fn resume_session(
        &self,
        token: impl AsRef<str>,
    ) -> Result<Arc<Session<UserId, Data>>, SessionError>
    where
        UserId: Clone + PartialEq,
        Data: Clone,
    {
        let token_str = token.as_ref();

        // 1. Check for token reuse conflict
        let conflict_user = self
            .used_tokens
            .get(token_str)
            .map(|entry| entry.value().1.clone());

        if let Some(user_id) = conflict_user {
            self.terminate_all_for_user(&user_id);
            return Err(SessionError::Conflict);
        }

        // 2. Fetch active session
        let session = self
            .sessions_by_token
            .get(token_str)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or(SessionError::NotFound)?;

        // 3. Verify session TTL
        if session.last_seen.elapsed() >= self.config.session_ttl {
            self.terminate_session(&session.session_id);
            return Err(SessionError::Expired);
        }

        // 4. Rotate token
        let new_token = Uuid::new_v4().to_string();
        let updated_session = Arc::new(Session {
            session_id: session.session_id,
            user_id: session.user_id.clone(),
            current_token: new_token.clone(),
            last_seen: Instant::now(),
            data: session.data.clone(),
        });

        self.used_tokens.insert(
            token_str.to_string(),
            (session.session_id, session.user_id.clone(), Instant::now()),
        );
        self.sessions_by_token.remove(token_str);
        self.sessions_by_token
            .insert(new_token, Arc::clone(&updated_session));
        self.sessions_by_id
            .insert(session.session_id, Arc::clone(&updated_session));

        Ok(updated_session)
    }

    /// Validate a token without rotation and update its `last_seen` timestamp.
    pub fn validate_and_touch(
        &self,
        token: impl AsRef<str>,
    ) -> Result<Arc<Session<UserId, Data>>, SessionError>
    where
        UserId: Clone,
        Data: Clone,
    {
        let token_str = token.as_ref();
        let session = self
            .sessions_by_token
            .get(token_str)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or(SessionError::NotFound)?;

        if session.last_seen.elapsed() >= self.config.session_ttl {
            self.terminate_session(&session.session_id);
            return Err(SessionError::Expired);
        }

        let updated_session = Arc::new(Session {
            last_seen: Instant::now(),
            session_id: session.session_id,
            user_id: session.user_id.clone(),
            current_token: session.current_token.clone(),
            data: session.data.clone(),
        });

        self.sessions_by_id
            .insert(session.session_id, Arc::clone(&updated_session));
        self.sessions_by_token
            .insert(session.current_token.clone(), Arc::clone(&updated_session));

        Ok(updated_session)
    }

    /// Retrieve a session by token.
    pub fn get_session_by_token(
        &self,
        token: impl AsRef<str>,
    ) -> Option<Arc<Session<UserId, Data>>> {
        self.sessions_by_token
            .get(token.as_ref())
            .map(|entry| Arc::clone(entry.value()))
    }

    /// Retrieve a session by session ID.
    pub fn get_session_by_id(&self, session_id: &Uuid) -> Option<Arc<Session<UserId, Data>>> {
        self.sessions_by_id
            .get(session_id)
            .map(|entry| Arc::clone(entry.value()))
    }

    /// Update `last_seen` for a session by ID.
    pub fn update_last_seen(&self, session_id: &Uuid)
    where
        UserId: Clone,
        Data: Clone,
    {
        let session_data = self
            .sessions_by_id
            .get(session_id)
            .map(|entry| Arc::clone(entry.value()));

        if let Some(session) = session_data {
            let updated_session = Arc::new(Session {
                last_seen: Instant::now(),
                session_id: session.session_id,
                user_id: session.user_id.clone(),
                current_token: session.current_token.clone(),
                data: session.data.clone(),
            });

            self.sessions_by_id
                .insert(*session_id, Arc::clone(&updated_session));
            self.sessions_by_token
                .insert(updated_session.current_token.clone(), updated_session);
        }
    }

    /// Terminate and remove a session by its ID.
    pub fn terminate_session(&self, session_id: &Uuid) {
        if let Some((_, session)) = self.sessions_by_id.remove(session_id) {
            self.sessions_by_token.remove(&session.current_token);
            self.used_tokens.retain(|_, (sid, _, _)| sid != session_id);
        }
    }

    /// Terminate all sessions belonging to a specific user.
    pub fn terminate_all_for_user(&self, user_id: &UserId)
    where
        UserId: PartialEq,
    {
        let to_remove: Vec<Uuid> = self
            .sessions_by_id
            .iter()
            .filter(|entry| entry.value().user_id == *user_id)
            .map(|entry| *entry.key())
            .collect();

        for sid in to_remove {
            self.terminate_session(&sid);
        }
    }

    /// Clean up expired sessions and old used tokens.
    pub fn cleanup(&self) {
        let used_token_ttl = self.config.used_token_ttl;
        let session_ttl = self.config.session_ttl;

        self.used_tokens
            .retain(|_, (_, _, time)| time.elapsed() < used_token_ttl);

        let to_remove: Vec<Uuid> = self
            .sessions_by_id
            .iter()
            .filter(|entry| entry.value().last_seen.elapsed() >= session_ttl)
            .map(|entry| *entry.key())
            .collect();

        for sid in to_remove {
            self.terminate_session(&sid);
        }
    }
}

#[async_trait]
impl<UserId, Data> SessionStore<UserId, Data> for SessionManager<UserId, Data>
where
    UserId: Clone + PartialEq + Send + Sync + 'static,
    Data: Clone + Send + Sync + 'static,
{
    async fn create_session(&self, user_id: UserId, data: Data) -> Arc<Session<UserId, Data>> {
        self.create_session(user_id, data)
    }

    async fn resume_session(
        &self,
        token: &str,
    ) -> Result<Arc<Session<UserId, Data>>, SessionError> {
        self.resume_session(token)
    }

    async fn validate_and_touch(
        &self,
        token: &str,
    ) -> Result<Arc<Session<UserId, Data>>, SessionError> {
        self.validate_and_touch(token)
    }

    async fn get_session_by_token(&self, token: &str) -> Option<Arc<Session<UserId, Data>>> {
        self.get_session_by_token(token)
    }

    async fn get_session_by_id(&self, session_id: &Uuid) -> Option<Arc<Session<UserId, Data>>> {
        self.get_session_by_id(session_id)
    }

    async fn update_last_seen(&self, session_id: &Uuid) {
        self.update_last_seen(session_id);
    }

    async fn terminate_session(&self, session_id: &Uuid) {
        self.terminate_session(session_id);
    }

    async fn terminate_all_for_user(&self, user_id: &UserId) {
        self.terminate_all_for_user(user_id);
    }

    async fn cleanup(&self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let manager = SessionManager::<Uuid, ()>::new();
        let user_id = Uuid::new_v4();

        // 1. Create session
        let session1 = manager.create_session(user_id, ());
        let token1 = session1.current_token.clone();
        assert!(manager.sessions_by_token.contains_key(&token1));
        assert_eq!(manager.active_session_count(), 1);

        // 2. Resume session (token rotation)
        let session2 = manager.resume_session(&token1).expect("Should resume");
        let token2 = session2.current_token.clone();

        assert_ne!(token1, token2);
        assert!(manager.used_tokens.contains_key(&token1));
        assert!(manager.sessions_by_token.contains_key(&token2));
        assert!(!manager.sessions_by_token.contains_key(&token1));

        // 3. Replay attack (double use of old token)
        let res = manager.resume_session(&token1);
        assert_eq!(res, Err(SessionError::Conflict));
        assert_eq!(manager.active_session_count(), 0);
    }

    #[test]
    fn test_session_expiration() {
        let config = SessionConfig {
            session_ttl: Duration::from_millis(10),
            used_token_ttl: Duration::from_millis(50),
        };
        let manager = SessionManager::<Uuid, ()>::with_config(config);
        let user_id = Uuid::new_v4();

        let session = manager.create_session(user_id, ());
        std::thread::sleep(Duration::from_millis(15));

        let res = manager.resume_session(&session.current_token);
        assert_eq!(res, Err(SessionError::Expired));
        assert_eq!(manager.active_session_count(), 0);
    }

    #[test]
    fn test_validate_and_touch() {
        let manager = SessionManager::<Uuid, String>::new();
        let user_id = Uuid::new_v4();

        let session = manager.create_session(user_id, "extra_state".to_string());
        assert_eq!(session.data, "extra_state");

        let validated = manager
            .validate_and_touch(&session.current_token)
            .expect("Should validate");
        assert_eq!(validated.current_token, session.current_token);
        assert_eq!(&*validated.data, "extra_state");
        assert_eq!(&**validated, "extra_state");
    }
}
