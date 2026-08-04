use notifiapp_protocol_core::{
    AuthSessionEvent, CredentialsAuth, ParsedAuthResponse, PostcardCodec, ProtocolCodec,
    TypedAuthWrapper,
};
use notifiapp_transport::{AuthHandler, AuthOutcome};
use std::sync::Arc;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
enum TestAction {
    Login { user: String, pass: String },
    Resume { token: String },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
enum TestResponse {
    Ok { token: String },
    Expired,
    Failed,
}

#[tokio::test]
async fn test_auth_session_events() {
    let login_builder = |u, p| TestAction::Login { user: u, pass: p };
    let resume_builder = |t| TestAction::Resume { token: t };
    let response_parser = |resp| match resp {
        TestResponse::Ok { token } => ParsedAuthResponse::Success { token: Some(token) },
        TestResponse::Expired => ParsedAuthResponse::SessionExpired,
        TestResponse::Failed => ParsedAuthResponse::Unauthorized,
    };

    let credentials_auth = Arc::new(CredentialsAuth::<TestAction, TestResponse>::new(
        login_builder,
        resume_builder,
        response_parser,
    ));

    let handler =
        TypedAuthWrapper::<TestAction, TestResponse, PostcardCodec>::new(credentials_auth.clone());

    let mut event_rx = handler.subscribe_session_events();

    // 1. Trigger Login (attempted_resume = false)
    credentials_auth
        .set_credentials("user".to_string(), "pass".to_string())
        .await;

    // Simulate server response for login success
    let success_resp = TestResponse::Ok {
        token: "token_123".to_string(),
    };
    let success_bytes = PostcardCodec::serialize(&success_resp).unwrap();
    let outcome = handler.process_auth_response(&success_bytes).await;

    assert!(matches!(outcome, AuthOutcome::Success));
    assert_eq!(
        event_rx.recv().await.unwrap(),
        AuthSessionEvent::NewSessionCreated
    );

    // 2. Trigger Resume (attempted_resume = true)
    let success_resp_resume = TestResponse::Ok {
        token: "token_456".to_string(),
    };
    let success_bytes_resume = PostcardCodec::serialize(&success_resp_resume).unwrap();
    let outcome_resume = handler.process_auth_response(&success_bytes_resume).await;

    assert!(matches!(outcome_resume, AuthOutcome::Success));
    assert_eq!(
        event_rx.recv().await.unwrap(),
        AuthSessionEvent::SessionResumed
    );

    // 3. Trigger SessionExpired (SessionRestoreFailed)
    let expired_resp = TestResponse::Expired;
    let expired_bytes = PostcardCodec::serialize(&expired_resp).unwrap();
    let outcome_expired = handler.process_auth_response(&expired_bytes).await;

    assert!(matches!(outcome_expired, AuthOutcome::Failed));
    assert_eq!(
        event_rx.recv().await.unwrap(),
        AuthSessionEvent::SessionRestoreFailed
    );
}
