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
