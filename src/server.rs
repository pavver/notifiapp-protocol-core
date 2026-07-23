use crate::codec::ProtocolCodec;
use crate::envelope::{RequestEnvelope, ResponseEnvelope};
use async_trait::async_trait;
use notifiapp_transport::{Frame, FrameKind, ServerSessionHandle, TransportError};
use std::sync::Arc;

/// A trait defining the application logic for handling protocol action requests on the server.
#[async_trait]
pub trait ProtocolHandler<State, Action, Response, Error>: Send + Sync + 'static {
    /// Process a single incoming action and return the result.
    async fn handle_action(&self, state: &State, action: Action) -> Result<Response, Error>;
}

/// A generic server-side helper to handle typed session message loops.
pub struct ProtocolServer<State, Action, Response, Error, Codec> {
    _phantom: std::marker::PhantomData<(State, Action, Response, Error, Codec)>,
}

impl<State, Action, Response, Error, Codec> ProtocolServer<State, Action, Response, Error, Codec>
where
    Action: for<'de> serde::Deserialize<'de> + Send + 'static,
    Response: serde::Serialize + Send + 'static,
    Error: serde::Serialize + Send + 'static,
    Codec: ProtocolCodec,
    State: Clone + Send + Sync + 'static,
{
    /// Run the server session loop, processing incoming messages from the transport inbox.
    ///
    /// Spawns concurrent tokio tasks for each request to avoid blocking the main session loop.
    pub async fn run_session(
        state: State,
        mut inbox: tokio::sync::mpsc::Receiver<Frame>,
        handle: ServerSessionHandle,
        handler: Arc<dyn ProtocolHandler<State, Action, Response, Error>>,
    ) {
        while let Some(frame) = inbox.recv().await {
            // We only handle message requests. Pings, Pongs and Events are managed by the transport layer.
            if frame.kind == FrameKind::Message {
                let state_clone = state.clone();
                let handle_clone = handle.clone();
                let handler_clone = Arc::clone(&handler);

                tokio::spawn(async move {
                    if let Err(e) =
                        Self::process_frame(state_clone, frame, handle_clone, handler_clone).await
                    {
                        tracing::error!("Failed to process session frame: {:?}", e);
                    }
                });
            }
        }
    }

    async fn process_frame(
        state: State,
        frame: Frame,
        handle: ServerSessionHandle,
        handler: Arc<dyn ProtocolHandler<State, Action, Response, Error>>,
    ) -> Result<(), TransportError> {
        // 1. Deserialize the incoming envelope
        let request: RequestEnvelope<Action> = match Codec::deserialize(&frame.data) {
            Ok(req) => req,
            Err(e) => {
                tracing::warn!("Failed to deserialize request frame: {:?}", e);
                return Err(TransportError::DecodeError(e.to_string()));
            }
        };

        // 2. Execute the business logic handler
        let outcome = handler.handle_action(&state, request.action).await;

        // 3. Build response envelope preserving request ID
        let response = ResponseEnvelope {
            id: request.id,
            payload: outcome,
        };

        // 4. Serialize response
        let resp_bytes = match Codec::serialize(&response) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed to serialize response envelope: {:?}", e);
                return Err(TransportError::EncodeError(e.to_string()));
            }
        };

        // 5. Send back to client via transport
        handle.respond(frame.id, resp_bytes)?;
        Ok(())
    }
}
