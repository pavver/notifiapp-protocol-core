# AGENTS.md — Technical details of the Core Protocol Layer

This module provides generic protocol building blocks for client-server communication.
Every sentence in this document is on a separate line.

## 1. Core Abstractions (`lib.rs`)

- Provides protocol-agnostic envelopes and types.
- Exports generic envelopes for requests, responses, and push events.
- Re-exports serialization codecs, subscription registry, and the endpoint fallback manager.

## 2. Generic Codecs (`codec.rs`)

- Defines the `ProtocolCodec` trait.
- Implements `PostcardCodec` for compact binary format.
- Implements `JsonCodec` for human-readable JSON format.

## 3. Communication Envelopes (`envelope.rs`)

- `RequestEnvelope<A>` wraps outgoing actions with a unique request ID.
- `ResponseEnvelope<R, E>` correlates replies using the matching request ID.
- `EventEnvelope<V>` wraps server-push events sent to subscribed client targets.

## 4. Priority Endpoint Fallback Manager (`endpoints.rs`)

- Implements priority-based fallback across multiple endpoints.
- Promotes the last successfully connected URL to the front of its priority tier.
- Allows forcing a specific connection URL using `switch_to_endpoint`.
- Emits events on config changes via `subscribe_changes`.

## 5. Session-based Authentication Handler (`auth.rs`)

- `SessionAuthHandler` automates token resumption and credential login.
- Emits `AuthSessionEvent` on success or failure of connection attempts.
- Triggers `SessionResumed` when a token successfully restores the session.
- Triggers `NewSessionCreated` when logging in from scratch.
- Triggers `SessionRestoreFailed` when the resumption token is rejected.

## 6. Server-Side Execution (`server.rs`)

- `ProtocolHandler` trait defines application-specific action handlers.
- `ProtocolServer` handles incoming websocket message frames concurrently.

## 7. Client Subscriptions (`subscriptions.rs`)

- `SubscriptionRegistry` coordinates active event callbacks.
- Allows registering callbacks per subscription ID and dispatching incoming events.

## 8. Incremental Diffing & Macros (`Diffable`)

- The project integrates macro-based incremental updates.
- Objects derive `Diffable` to automatically calculate field-level changes.
- Uses `#[diff(required)]` for fields like IDs that must always be sent in updates.
- Uses `#[diff(immutable)]` for fields like creation time that never change.
- Inlined nested Diffable structures are automatically resolved at compile time without extra attributes.
- Non-annotated fields fall back to full value replacement if changed.
