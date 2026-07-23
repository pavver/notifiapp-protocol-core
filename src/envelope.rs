use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Envelope for outgoing requests sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope<A> {
    /// Unique request identifier to correlate with responses.
    pub id: u32,
    /// The action payload containing operation details.
    pub action: A,
}

impl<A> RequestEnvelope<A> {
    /// Create a response envelope corresponding to this request.
    pub fn reply<R, E>(self, payload: Result<R, E>) -> ResponseEnvelope<R, E> {
        ResponseEnvelope {
            id: self.id,
            payload,
        }
    }

    /// Create a successful response envelope corresponding to this request.
    pub fn reply_ok<R, E>(self, data: R) -> ResponseEnvelope<R, E> {
        ResponseEnvelope {
            id: self.id,
            payload: Ok(data),
        }
    }
}

/// Envelope for responses returned from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope<R, E> {
    /// The matching request identifier.
    pub id: u32,
    /// The result of the operation, containing either success payload or error code.
    pub payload: Result<R, E>,
}

/// Envelope for server-push events dispatched to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<V> {
    /// Optional subscription identifier to route the event to specific callback.
    pub subscription_id: Option<Uuid>,
    /// The server-push event payload.
    pub event: V,
}

impl<V> EventEnvelope<V> {
    /// Create a new event envelope.
    pub fn new(subscription_id: impl Into<Option<Uuid>>, event: V) -> Self {
        Self {
            subscription_id: subscription_id.into(),
            event,
        }
    }
}
