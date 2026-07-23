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
