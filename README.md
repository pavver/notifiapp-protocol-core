# notifiapp-protocol-core

Core protocol abstractions for notifiapp: generic envelopes, clients, servers, and subscriptions.
Every sentence in this document is placed on a separate line.

## Overview

- Defines reusable elements of client-server messaging.
- Decouples serialization formats from network protocols.
- Implements generic priority-based endpoints fallback.
- Implements stateful token-based authentication lifecycle.

## Codecs

- Supports `PostcardCodec` (compact binary format).
- Supports `JsonCodec` (JSON text format).

## Multi-URL Failover

- Managed through `EndpointManager`.
- Supports manual override with `switch_to_endpoint`.
- Prioritizes local connections before remote ones.

## Auth Lifecycle

- Tracks states: `SessionResumed`, `NewSessionCreated`, and `SessionRestoreFailed`.
- Broadcasts session changes so clients can adjust subscriptions.

## Incremental Updates (Diffing)

- Integrates procedural macro `Diffable` for incremental struct diff calculations.
- Reduces network overhead by transmitting only updated fields.

## Quick Start Examples

### Endpoint Fallback Manager

```rust
use notifiapp_protocol_core::{EndpointManager, EndpointPriority};

let manager = EndpointManager::new();
let local_handle = manager.add_endpoint("ws://192.168.1.50/ws", EndpointPriority::Local).unwrap();
let remote_handle = manager.add_endpoint("ws://my.server.com", EndpointPriority::Remote).unwrap();

// Listen to configuration or fallback changes
let mut rx = manager.subscribe_changes();
tokio::spawn(async move {
    while rx.changed().await.is_ok() {
        println!("Endpoints list changed!");
    }
});

// Force specific URL or let priorities resolve
manager.switch_to_endpoint(&remote_handle).unwrap();
```

### Incremental Diffing (Diffable)

```rust
use notifiapp_protocol_core::Diffable;
use uuid::Uuid;

#[derive(Debug, Clone, Diffable, PartialEq)]
pub struct Device {
    #[diff(required)]
    pub id: Uuid,
    pub name: String,
    #[diff(immutable)]
    pub hardware_id: String,
}

let dev_old = Device { id: Uuid::new_v4(), name: "Old Name".into(), hardware_id: "HW123".into() };
let dev_new = Device { id: dev_old.id, name: "New Name".into(), hardware_id: "HW123".into() };

// Calculate incremental patch
if let Some(patch) = dev_old.diff(&dev_new) {
    // Send `patch` over the network
    println!("Changes detected: {:?}", patch);
}
```

### Direct Queries (Request-Response without Subscriptions)

```rust
// ProtocolClient supports direct request-response communication (e.g. for fetching history or one-off tasks)
// This does not require active subscriptions and returns the deserialized response directly.
let response: GetMessagesResponse = client.get_messages(GetMessagesQuery {
    room_id,
    cursor: None,
    limit: 50,
}).await.unwrap();
```

### Managing Subscriptions (High-Level Watch/Unwatch Flow)

```rust
// 1. Subscribe to events using a high-level watch helper.
// The helper sends the watch Action to the server, registers the callback locally,
// and returns a subscription token (Uuid) and initial data.
let (sub_id, initial_data) = client.watch_users(query, move |event: ServerEvent| {
    println!("Received user update: {:?}", event);
}).await.unwrap();

// 2. Unsubscribe by passing the subscription token back to the client.
// The helper automatically removes the local callback and informs the server.
client.unwatch(sub_id).await.unwrap();
```
