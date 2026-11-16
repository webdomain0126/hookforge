# HookForge

Reliable webhook delivery service written in Rust.

An app hands HookForge a message and a receiver URL. HookForge stores it, delivers it with an HMAC signature, retries with backoff when the receiver is down, and marks it Delivered or Dead.

> Status: work in progress. Built milestone by milestone.

## Roadmap

- [x] M1: Core types: delivery status, endpoint validation, unit tests
- [x] M2: PostgreSQL tables
- [x] M3: HTTP API for endpoints and events (Axum)
- [x] M4: Workers that claim and send deliveries (`FOR UPDATE SKIP LOCKED`)
- [ ] M5: HMAC-SHA256 signatures
- [ ] M6: Retries, exponential backoff, attempt log
- [ ] M7: Worker pool, crash recovery, graceful shutdown
- [ ] M8: Per-receiver rate limits (Redis)
- [ ] M9: Tracing, Docker, integration tests, docs

## Run the tests

    cargo test

## Tech

Rust, Tokio, Axum, SQLx, PostgreSQL, Serde, thiserror