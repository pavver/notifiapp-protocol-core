//! # notifiapp-protocol-core
//!
//! Generic, reusable protocol abstractions for notifiapp client-server communication.

pub mod auth;
pub mod client;
pub mod codec;
pub mod common;
pub mod conflated_queue;
pub mod endpoints;
pub mod envelope;
pub mod server;
pub mod subscriptions;
#[cfg(feature = "webrtc")]
pub mod webrtc;

// Re-exports for convenience
pub use auth::{AuthSessionEvent, ParsedAuthResponse, SessionAuthHandler};
pub use client::ProtocolClient;
pub use codec::{CodecError, JsonCodec, PostcardCodec, ProtocolCodec};
pub use common::{DataPage, Pagination, SortOrder};
pub use conflated_queue::{Conflatabled, ConflatedQueue, ConflationKey};
pub use endpoints::{EndpointData, EndpointHandle, EndpointManager, EndpointPriority};
pub use envelope::{EventEnvelope, RequestEnvelope, ResponseEnvelope};
pub use server::{ProtocolHandler, ProtocolServer};
pub use subscriptions::{DiffResult, ReactiveTracker, SubscriptionRegistry};
#[cfg(feature = "webrtc")]
pub use webrtc::{ParticipantState, RoomState, SignalPayload, TrackInfo, TrackType, WebRtcSignal};
