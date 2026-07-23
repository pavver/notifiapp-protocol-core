# RULES.md — Coding Standards and Constraints for Core Protocol Layer

This document defines the strict development rules for `notifiapp-protocol-core`.
Every sentence in this document is placed on a separate line.

## 1. General Principles

- Minimize external dependencies to keep the core library lightweight and fast.
- All public types must be fully documented with docstrings in English.
- Avoid any workspace-specific or application-specific structures in the core library.

## 2. Rust Code Guidelines

- Never use `.unwrap()` or `.expect()` under any circumstances.
- Propagate all errors using `Result` and the `?` operator.
- Write unit tests for every new module inside the `tests/` directory.
- Use `thiserror` for library-level custom errors.
- Ensure all structures implement `Send + Sync + 'static` for multi-threaded tokio environments.
- Always run `cargo clippy` and `cargo fmt` before submitting changes.

## 3. Protocol and Codec Rules

- Codes and endpoints must be agnostic of the underlying serialization protocol.
- Support both JSON (`JsonCodec`) and Postcard (`PostcardCodec`) codecs.
- Envelopes must preserve request IDs to properly correlate asynchronous frames.

## 4. Multi-URL Endpoint Handling

- Keep the `EndpointManager` thread-safe using parking_lot RwLocks.
- Ensure forced override endpoints take absolute precedence over sorted priority tiers.
- Emit change events on every endpoint list or state modification.

## 5. Auth and Session Lifecycles

- Keep the `SessionAuthHandler` generic over `Action` and `Response`.
- Always emit `AuthSessionEvent` on authentication success or failure.
- Explicitly differentiate between session resumption and credentials logins.
