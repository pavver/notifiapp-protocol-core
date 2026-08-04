//! # notifiapp-protocol-core
//!
//! Generic, reusable protocol abstractions for notifiapp client-server communication.

pub mod auth;
pub mod client;

#[cfg(feature = "build-utils")]
pub mod build_utils;

/// Compile-time validation of protocol string (only alphanumeric, '.', '-', '_')
pub const fn validate_protocol_string(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let valid = (b >= b'a' && b <= b'z')
            || (b >= b'A' && b <= b'Z')
            || (b >= b'0' && b <= b'9')
            || b == b'.'
            || b == b'-'
            || b == b'_';
        if !valid {
            panic!(
                "Invalid character in protocol string (only alphanumeric, '.', '-', '_' are allowed)"
            );
        }
        i += 1;
    }
    s
}
pub mod codec;
pub mod common;
pub mod conflated_queue;
pub mod diff;
pub mod endpoints;
pub mod envelope;
pub mod server;
pub mod subscriptions;

// Re-exports for convenience
pub use auth::{
    AuthProtocol, AuthSessionEvent, CredentialsAuth, ParsedAuthResponse, StaticTokenAuth,
    TypedAuthWrapper,
};
pub use client::ProtocolClient;
pub use codec::{CodecError, JsonCodec, PostcardCodec, ProtocolCodec};
pub use common::{DataPage, Pagination, SortOrder};
pub use conflated_queue::{Conflatabled, ConflatedQueue, ConflationKey};
pub use diff::{Diffable, GetPatchType};
pub use endpoints::{EndpointData, EndpointHandle, EndpointManager, EndpointPriority};
pub use envelope::{EventEnvelope, RequestEnvelope, ResponseEnvelope};
pub use notifiapp_protocol_macros::Diffable;
pub use server::{ProtocolHandler, ProtocolServer};
pub use subscriptions::{DiffResult, ReactiveTracker, SubscriptionRegistry};
