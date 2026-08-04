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
    pub fn new(id: u32, action: A) -> Self {
        Self { id, action }
    }

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

impl<V: crate::conflated_queue::Conflatabled + Clone> crate::conflated_queue::Conflatabled
    for EventEnvelope<V>
{
    fn conflation_key(&self) -> Option<crate::conflated_queue::ConflationKey> {
        let inner_key = self.event.conflation_key()?;
        let sub_str = self
            .subscription_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "global".to_string());

        // Convert the inner key into a string and prepend subscription
        let inner_key_str = match inner_key {
            crate::conflated_queue::ConflationKey::Unique(_) => return None, // Don't conflate unique keys
            crate::conflated_queue::ConflationKey::Entity(s, _) => s,
            crate::conflated_queue::ConflationKey::Custom(s) => s,
        };

        Some(crate::conflated_queue::ConflationKey::Custom(format!(
            "{}_{}",
            sub_str, inner_key_str
        )))
    }

    fn merge_with(&self, newer: &Self) -> Option<Self> {
        if self.subscription_id != newer.subscription_id {
            return Some(newer.clone());
        }

        if let Some(merged_event) = self.event.merge_with(&newer.event) {
            let mut result = newer.clone();
            result.event = merged_event;
            Some(result)
        } else {
            Some(newer.clone())
        }
    }
}
