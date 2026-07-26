use crate::codec::{CodecError, ProtocolCodec};
use crate::envelope::{EventEnvelope, RequestEnvelope, ResponseEnvelope};
use crate::subscriptions::SubscriptionRegistry;
use notifiapp_transport::{MessagePriority, Transport, TransportError};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use thiserror::Error;

/// Errors that can occur during protocol client operations.
#[derive(Debug, Error)]
pub enum ProtocolError<E> {
    /// An error in the underlying transport layer.
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// A serialization or deserialization error.
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),

    /// An application-specific error returned by the server.
    #[error("Protocol error: {0:?}")]
    Protocol(E),
}

/// A trait for actions that have an associated message priority.
pub trait Prioritized {
    /// Return the priority for this message.
    fn priority(&self) -> MessagePriority;
}

/// A generic typed protocol client.
///
/// Wraps `notifiapp_transport::WsClient` and handles:
/// - Encoding request actions into `RequestEnvelope` using codec `C`.
/// - Sending requests via transport with proper priority scheduling.
/// - Decoding response bytes into `ResponseEnvelope` and extracting outcomes.
/// - Listening for server-push events, decoding them, and dispatching to `SubscriptionRegistry`.
pub struct ProtocolClient<Action, Response, Error, Event, Codec> {
    transport: Arc<dyn Transport>,
    subscriptions: Arc<SubscriptionRegistry<Event>>,
    next_request_id: AtomicU32,
    _phantom: std::marker::PhantomData<(Action, Response, Error, Codec)>,
}

impl<Action, Response, Error, Event, Codec> ProtocolClient<Action, Response, Error, Event, Codec>
where
    Action: serde::Serialize + Send + Sync + 'static,
    Response: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Error: for<'de> serde::Deserialize<'de> + std::fmt::Debug + Send + Sync + 'static,
    Event: for<'de> serde::Deserialize<'de> + Clone + Send + Sync + 'static,
    Codec: ProtocolCodec,
{
    /// Create a new protocol client wrapping the given transport client.
    ///
    /// This will automatically register an event handler on the transport to
    /// decode and dispatch server push events.
    pub fn new(transport: Arc<dyn Transport>) -> Arc<Self> {
        let subscriptions = Arc::new(SubscriptionRegistry::new());
        let subs_clone = Arc::clone(&subscriptions);

        transport.on_event(Arc::new(move |bytes| {
            if let Ok(EventEnvelope {
                subscription_id: Some(sub_id),
                event,
            }) = Codec::deserialize::<EventEnvelope<Event>>(&bytes)
            {
                subs_clone.dispatch(&sub_id, event);
            }
        }));

        Arc::new(Self {
            transport,
            subscriptions,
            next_request_id: AtomicU32::new(1),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Access the subscription registry to register or remove event callbacks.
    pub fn subscriptions(&self) -> &SubscriptionRegistry<Event> {
        &self.subscriptions
    }

    /// Access the underlying transport client.
    pub fn transport(&self) -> &Arc<dyn Transport> {
        &self.transport
    }

    /// Subscribe to connection state changes.
    pub fn subscribe_state(
        &self,
    ) -> tokio::sync::watch::Receiver<notifiapp_transport::ConnectionState> {
        self.transport.subscribe_state()
    }

    /// Send an action request with a custom priority and await the response.
    pub async fn call_with_priority(
        &self,
        action: Action,
        priority: MessagePriority,
    ) -> Result<Response, ProtocolError<Error>> {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let req = RequestEnvelope { id, action };

        let req_bytes = Codec::serialize(&req)?;
        let resp_bytes = self.transport.request(req_bytes, priority).await?;

        let resp: ResponseEnvelope<Response, Error> = Codec::deserialize(&resp_bytes)?;
        match resp.payload {
            Ok(data) => Ok(data),
            Err(err) => Err(ProtocolError::Protocol(err)),
        }
    }

    /// Send an action request and await the response.
    ///
    /// Automatically infers the message priority from the Action using the `Prioritized` trait.
    pub async fn call(&self, action: Action) -> Result<Response, ProtocolError<Error>>
    where
        Action: Prioritized,
    {
        let priority = action.priority();
        self.call_with_priority(action, priority).await
    }
}
