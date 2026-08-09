---
name: Protocol Architecture
description: Comprehensive guide for AI agents on creating large-scale, modular protocol projects similar to notifiapp-protocol, using notifiapp-protocol-core.
---

# Creating a Scalable Protocol (Protocol Architecture)

This skill describes how to build a robust client-server protocol crate based on `notifiapp-protocol-core`.
It strictly follows the architecture and patterns established by `notifiapp-protocol`.
Every sentence in this document is on a separate line.

## 1. Crate Setup and Dependencies

Create a library crate (`cargo new --lib your-protocol`).
Add the required dependencies to `Cargo.toml`.
Large protocols require a rich set of asynchronous and synchronization primitives.

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
postcard = { version = "1.0", features = ["use-std"] }
uuid = { version = "1.0", features = ["v4", "v5", "serde"] }
notifiapp-protocol-core = { git = "https://github.com/pavver/notifiapp-protocol-core" }
notifiapp-transport = { git = "https://github.com/pavver/notifiapp-transport" }
tokio = { version = "1.53", features = ["sync", "macros", "time", "rt"] }
tokio-util = "0.7"
futures-util = "0.3"
url = "2.5"
dashmap = "6.2"
parking_lot = "0.12"
anyhow = "1.0"

[build-dependencies]
notifiapp-protocol-core = { git = "https://github.com/pavver/notifiapp-protocol-core", features = ["build-utils"] }
```

Create a `build.rs` file at the crate root to generate protocol metadata.

```rust
fn main() {
    notifiapp_protocol_core::build_utils::configure_protocol_build();
}
```

## 2. Core Protocol File (`src/lib.rs`)

The `lib.rs` file acts as the central registry for the protocol.
It defines global constants, connection states, and aggregates module-specific enums.

```rust
pub mod client;
pub mod users;
pub mod devices;
pub mod topics;

pub const APP_NAME: &str = notifiapp_protocol_core::validate_protocol_string(env!("PROTOCOL_NAME"));
pub const APP_VERSION: &str = notifiapp_protocol_core::validate_protocol_string(env!("PROTOCOL_VERSION"));

use notifiapp_protocol_core::diff::Diffable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Handshaking,
    Authenticating,
    Resuming,
    Online,
    Unauthorized,
    ConnectionError(String),
    VersionMismatch { client: String, server: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    AuthFailed,
    SessionExpired,
    SessionConflict,
    AccessDenied,
    NotFound,
    ValidationError(String),
    InternalError,
    Timeout,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
}

pub type RequestEnvelope = notifiapp_protocol_core::RequestEnvelope<Action>;
pub type ResponseEnvelope = notifiapp_protocol_core::ResponseEnvelope<ResponseData, ErrorCode>;
pub type EventEnvelope = notifiapp_protocol_core::EventEnvelope<ServerEvent>;
```

## 3. Modular Architecture (Domain Modules)

Domain modules (e.g. `users`, `devices`) must be split into two files: `mod.rs` and `protocol.rs`.

### Data Models (`mod.rs`)

The `mod.rs` file contains the data structures, patch models, and imports.
Leaf enums used within `Diffable` structs must use the `impl_value_diff!` macro.

```rust
// src/users/mod.rs
use notifiapp_protocol_core::diff::Diffable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod protocol;
pub use protocol::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    User,
    Guest,
}

// Crucial: Implement Diffable for custom leaf enums
notifiapp_protocol_core::impl_value_diff!(UserRole);

#[derive(Clone, Debug, PartialEq, Diffable, Serialize, Deserialize)]
pub struct UserInfo {
    #[diff(key)]
    pub id: Uuid,
    
    #[diff(immutable)]
    pub created_at: u64,
    
    pub name: String,
    pub role: UserRole,
}
```

### Protocol Enums (`protocol.rs`)

The `protocol.rs` file defines the request, response, and event enums for the domain.

```rust
// src/users/protocol.rs
use super::UserInfo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum UserAction {
    Create(UserInfo),
    Delete(Uuid),
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum UserResponse {
    Created(Uuid),
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum UserEvent {
    LoginSuccess(Uuid),
}
```

## 4. Aggregating Actions, Responses, and Events

In `src/lib.rs`, aggregate all domain-specific enums into global enums.
Include the Watch/Subscribe pattern for collections.

```rust
// src/lib.rs (continued)

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum Action {
    Ping,
    Pong,
    Login { user: String, pass: String },
    User(users::UserAction),
    Device(devices::DeviceAction),
    // Watch pattern for subscriptions
    WatchUsers,
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum ResponseData {
    Ok,
    Pong,
    LoginSuccess { token: String, user_id: Uuid },
    User(users::UserResponse),
    Device(devices::DeviceResponse),
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum ServerEvent {
    Ping,
    SystemAlert { message: String, level: AlertLevel },
    ItemAdded(TypedData),
    ItemUpdated(TypedPatch),
    ItemRemoved(Uuid),
    CollectionUpdate {
        added: Vec<TypedData>,
        removed: Vec<Uuid>,
    },
    User(users::UserEvent),
    Topic(topics::TopicEvent),
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum TypedData {
    User(users::UserInfo),
    Device(devices::DeviceInfo),
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum TypedPatch {
    User(users::UserInfoPatch),
    Device(devices::DeviceInfoPatch),
}
```

## 5. Event Conflation (`Conflatabled`)

The global `ServerEvent` must implement `Conflatabled` to reduce network congestion.
The `conflation_key` must include a type prefix to prevent collisions between different types with identical IDs.

```rust
// src/lib.rs (continued)

impl notifiapp_protocol_core::conflated_queue::Conflatabled for ServerEvent {
    fn conflation_key(&self) -> Option<notifiapp_protocol_core::conflated_queue::ConflationKey> {
        match self {
            ServerEvent::ItemUpdated(patch) => {
                let (type_str, id) = match patch {
                    TypedPatch::User(p) => ("user", p.id.to_string()),
                    TypedPatch::Device(p) => ("device", p.id.to_string()),
                };
                Some(notifiapp_protocol_core::conflated_queue::ConflationKey::Custom(
                    format!("updated_{}_{}", type_str, id)
                ))
            }
            ServerEvent::CollectionUpdate { .. } => {
                Some(notifiapp_protocol_core::conflated_queue::ConflationKey::Custom(
                    "collection_update".to_string()
                ))
            }
            _ => None
        }
    }

    fn merge_with(&self, newer: &Self) -> Option<Self> {
        match (self, newer) {
            (ServerEvent::ItemUpdated(self_patch), ServerEvent::ItemUpdated(other_patch)) => {
                let mut merged = self_patch.clone();
                match (&mut merged, other_patch) {
                    (TypedPatch::User(s), TypedPatch::User(o)) => {
                        crate::users::UserInfo::merge_patch(s, o)
                    }
                    (TypedPatch::Device(s), TypedPatch::Device(o)) => {
                        crate::devices::DeviceInfo::merge_patch(s, o)
                    }
                    _ => return Some(newer.clone()),
                }
                Some(ServerEvent::ItemUpdated(merged))
            }
            (ServerEvent::CollectionUpdate { added: s_add, removed: s_rem },
             ServerEvent::CollectionUpdate { added: o_add, removed: o_rem }) => {
                let mut m_add = s_add.clone();
                let mut m_rem = s_rem.clone();
                m_add.extend(o_add.clone());
                m_rem.extend(o_rem.clone());
                Some(ServerEvent::CollectionUpdate { added: m_add, removed: m_rem })
            }
            _ => Some(newer.clone()),
        }
    }
}
```

## 6. Message Prioritization

Implement `notifiapp_protocol_core::client::Prioritized` for the global `Action` enum.
This maps actions to transport WFQ priorities, ensuring critical operations like audio streaming are processed before normal requests.
The implementation relies on `notifiapp_transport::MessagePriority`.

```rust
// src/lib.rs (continued)
use notifiapp_transport::MessagePriority;

impl notifiapp_protocol_core::client::Prioritized for Action {
    fn priority(&self) -> MessagePriority {
        match self {
            // Example of a high-priority action:
            // Action::Audio(audio::AudioAction::Frame { .. }) => MessagePriority::RealTime,
            _ => MessagePriority::Normal,
        }
    }
}
```

## 7. The Client Module (`src/client/mod.rs`)

A complete protocol must expose a `client/` module.
This module typically contains a primary structure (e.g., `NotifiClient`).
This structure serves as the main entry point for client applications.
It encapsulates the transport (`WsClient`), authentication (`CredentialsAuth`), endpoints manager (`EndpointManager`), and the core protocol logic (`ProtocolClient`).
The client is responsible for mapping internal core errors to the protocol's public `ErrorCode`.

```rust
// src/client/mod.rs
use std::sync::Arc;
use crate::{Action, ErrorCode, RequestEnvelope, ResponseData, ResponseEnvelope, ServerEvent};

pub struct NotifiClient {
    core: Arc<
        notifiapp_protocol_core::client::ProtocolClient<
            Action,
            ResponseData,
            ErrorCode,
            ServerEvent,
            notifiapp_protocol_core::codec::PostcardCodec,
        >,
    >,
    auth: Arc<notifiapp_protocol_core::CredentialsAuth<RequestEnvelope, ResponseEnvelope>>,
    transport: Arc<notifiapp_transport::ws::WsClient>,
    pub endpoints: Arc<notifiapp_protocol_core::endpoints::EndpointManager>,
    cancel_token: tokio_util::sync::CancellationToken,
}

impl NotifiClient {
    pub fn new() -> Arc<Self> {
        let auth = Arc::new(notifiapp_protocol_core::CredentialsAuth::new(
            |user, pass| RequestEnvelope::new(1, Action::Login { user, pass }),
            |token| RequestEnvelope::new(1, Action::Resume { token }),
            |resp: ResponseEnvelope| match resp.payload {
                Ok(ResponseData::LoginSuccess { token, .. }) => notifiapp_protocol_core::ParsedAuthResponse::Success { token: Some(token) },
                Err(ErrorCode::SessionExpired) => notifiapp_protocol_core::ParsedAuthResponse::SessionExpired,
                _ => notifiapp_protocol_core::ParsedAuthResponse::Unauthorized,
            },
        ));

        let auth_wrapper = Arc::new(notifiapp_protocol_core::TypedAuthWrapper::<
            RequestEnvelope, ResponseEnvelope, notifiapp_protocol_core::codec::PostcardCodec,
        >::new(auth.clone()));

        let config = notifiapp_transport::ws::WsClientConfig::new(crate::APP_NAME, crate::APP_VERSION);
        let transport = notifiapp_transport::ws::WsClient::new(
            config,
            Some(auth_wrapper as Arc<dyn notifiapp_transport::AuthHandler>),
        );
        let core = notifiapp_protocol_core::client::ProtocolClient::new(
            transport.clone() as Arc<dyn notifiapp_transport::Transport>
        );

        let endpoints = Arc::new(notifiapp_protocol_core::endpoints::EndpointManager::new());
        let cancel_token = tokio_util::sync::CancellationToken::new();
        
        let client = Arc::new(Self { core, auth, transport, endpoints, cancel_token });
        
        // Note: A background task must be spawned here to synchronize endpoints with the transport state.
        // See notifiapp-protocol/src/client/mod.rs for the full implementation of this task.
        
        client
    }

    pub fn status(&self) -> notifiapp_transport::state::ConnectionState {
        self.transport.subscribe_state().borrow().clone()
    }

    pub fn subscribe_status(
        &self,
    ) -> tokio::sync::watch::Receiver<notifiapp_transport::state::ConnectionState> {
        self.core.subscribe_state()
    }

    pub(crate) async fn call(&self, action: Action) -> Result<ResponseData, ErrorCode> {
        self.core.call(action).await.map_err(|e| {
            match e {
                notifiapp_protocol_core::client::ProtocolError::Transport(t) => {
                    match t {
                        notifiapp_transport::TransportError::RequestTimeout => ErrorCode::Timeout,
                        _ => ErrorCode::InternalError,
                    }
                },
                notifiapp_protocol_core::client::ProtocolError::Protocol(e) => e,
                notifiapp_protocol_core::client::ProtocolError::Codec(_) => ErrorCode::InternalError,
            }
        })
    }
}
```

## 8. Client API Wrappers and Subscriptions

The `NotifiClient` structure acts as a facade. It is best practice to expose helper methods in submodules (e.g. `src/client/users.rs`) that call `self.call(...)` and return typed data.

For real-time events, you MUST expose `watch_...` methods that leverage the `SubscriptionRegistry`.
Do not make UI frameworks (like Leptos) interact with the `WsClient` directly.
Instead, the client wrappers register the provided callback using `self.core.subscriptions().register(...)`.

```rust
// src/client/users.rs
use super::NotifiClient;
use crate::*;
use uuid::Uuid;

impl NotifiClient {
    pub async fn watch_users<F>(
        &self,
        query: users::UserQuery,
        callback: F,
    ) -> Result<(Uuid, users::UserSubscriptionData), ErrorCode>
    where
        F: Fn(ServerEvent) + Send + Sync + 'static,
    {
        match self.call(Action::User(users::UserAction::Watch(query))).await {
            Ok(ResponseData::User(users::UserResponse::SubscriptionStarted { id, initial_data })) => {
                // Automatically route incoming events for this subscription to the callback
                self.core.subscriptions().register(id, callback);
                Ok((id, initial_data))
            }
            _ => Err(ErrorCode::InternalError),
        }
    }

    pub async fn unwatch(&self, sub_id: Uuid) -> Result<(), ErrorCode> {
        match self.call(Action::Unsubscribe(sub_id)).await {
            Ok(ResponseData::Ok) => {
                self.core.subscriptions().remove(&sub_id);
                Ok(())
            }
            _ => Err(ErrorCode::InternalError),
        }
    }
}
```

### UI Integration Note

When building an application using a framework like Leptos:
1. Initialize `NotifiClient` once.
2. In your UI components, call the `watch_...` method and inside the callback update your reactive state (e.g., `RwSignal`).
3. Always store the returned subscription ID (`sub_id`).
4. **Crucially**, implement a cleanup mechanism (like `on_cleanup` in Leptos or `Drop` in standard Rust) that calls `client.unwatch(sub_id).await`. This ensures that when a component is unmounted (e.g. the user closes a panel), the server stops sending those events.
```
