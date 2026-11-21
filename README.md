# HookForge

[![CI](https://github.com/webdomain0126/hookforge/actions/workflows/ci.yml/badge.svg)](https://github.com/webdomain0126/hookforge/actions/workflows/ci.yml)

A reliable webhook delivery service written in Rust.

An app hands HookForge a message and a receiver URL. HookForge stores it in PostgreSQL, signs it with HMAC-SHA256, delivers it, retries with exponential backoff when the receiver is down, and marks it **delivered** or **dead**. No message is lost, even if HookForge crashes mid-delivery.

## Features

- **Durable queue:** every delivery is stored in PostgreSQL before anything is sent
- **Concurrent workers:** a pool of workers claims jobs with `SELECT … FOR UPDATE SKIP LOCKED`, so no delivery is ever claimed by two workers at once
- **Signed payloads:** HMAC-SHA256 over `timestamp.body`, so receivers can reject forged, tampered or replayed messages
- **Retries with backoff:** 30s, 60s, 120s, 240s… capped at 1 hour; marked `dead` after 5 attempts
- **Attempt log:** every try is recorded with status code, error and duration
- **Crash recovery:** claims are leases; a reaper returns stuck deliveries to the queue
- **Per-endpoint rate limits:** never sends more than an endpoint's `max_per_second`
- **Graceful shutdown:** on Ctrl+C, workers finish their current delivery before exiting
- **CI:** GitHub Actions applies migrations to a fresh PostgreSQL, builds and runs the tests on every push

## How it works

```
App ──POST /events──▶ API ──INSERT──▶ PostgreSQL (deliveries: pending)
                                             │
                     workers claim with FOR UPDATE SKIP LOCKED
                                             ▼
                      sign (HMAC) ──POST──▶ Receiver URL
                                             │
               2xx ──▶ delivered    error ──▶ retry with backoff ──▶ dead after 5
```

Delivery is **at-least-once**: a receiver may occasionally get the same message twice (for example after a crash), so receivers should handle duplicates safely.

## Quick start

Requirements: Rust (stable) and PostgreSQL.

1. Create a database named `hookforge`.
2. Create a `.env` file in the project folder:

   ```
   DATABASE_URL=postgres://postgres:YOUR_PASSWORD@localhost:5432/hookforge
   ```

3. Apply the migrations in order:

   ```bash
   for f in migrations/*.sql; do psql "$DATABASE_URL" -f "$f"; done
   ```

4. Run it:

   ```bash
   cargo run
   ```

HookForge listens on `http://127.0.0.1:3000` and starts 4 workers and a reaper.

## API

### Register an endpoint

```bash
curl -X POST http://127.0.0.1:3000/endpoints \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com/hook","secret":"0123456789abcdef","max_per_second":10}'
```

Response `201 Created`: `{"id": 1}`

Rules: `url` must use `https://`, `secret` must be at least 16 characters, `max_per_second` must be 1 to 1000. Invalid input returns `400` with a clear message.

### Send an event

```bash
curl -X POST http://127.0.0.1:3000/events \
  -H "Content-Type: application/json" \
  -d '{"endpoint_id":1,"payload":{"type":"order.paid","order_id":123}}'
```

Response `202 Accepted`: `{"delivery_id": 1, "status": "pending"}`

An unknown `endpoint_id` returns `404`.

### Health check

`GET /health` returns `ok`.

## Verifying signatures (receiver side)

Every delivery carries two headers:

```
X-HookForge-Timestamp: 1794990630
X-HookForge-Signature: v1=732a687ec2de84f1...
```

The signature is `HMAC-SHA256(secret, "{timestamp}.{raw_body}")` in hex. Example in Python:

```python
import hashlib, hmac, time

def verify(secret: str, timestamp: str, raw_body: bytes, signature: str) -> bool:
    expected = "v1=" + hmac.new(
        secret.encode(), timestamp.encode() + b"." + raw_body, hashlib.sha256
    ).hexdigest()
    fresh = abs(time.time() - int(timestamp)) < 300  # reject messages older than 5 minutes
    return fresh and hmac.compare_digest(expected, signature)
```

## Project structure

```
src/
  main.rs        API routes, startup, graceful shutdown
  endpoint.rs    endpoint input and validation
  delivery.rs    delivery status
  worker.rs      claim, sign, send, log, retry
  retry.rs       backoff and give-up rules
  signature.rs   HMAC-SHA256 signing
  ratelimit.rs   per-endpoint rate limiter
  reaper.rs      crash recovery
migrations/      PostgreSQL schema
```

## Design decisions

- **PostgreSQL as the queue.** One system to run and back up, and `SKIP LOCKED` gives safe concurrent claiming without a separate message broker.
- **Leases instead of long transactions.** A claim holds a 60-second lease rather than an open database transaction during the HTTP call, so slow receivers don't hold database locks.
- **Rate limited is not a failure.** A rate-limited delivery is released without counting an attempt, so busy receivers don't push messages toward `dead`.

## Limitations and next steps

Honest notes on what a production version would add:

- Rate limits are in-process; running several HookForge instances would need a shared store such as Redis.
- Endpoint secrets are stored as plain text; they should be encrypted at rest.
- The API has no authentication yet.
- Logging uses `println!`; structured logs with `tracing` would help in production.
- A Docker image and integration tests against a live database.

## Tech

Rust · Tokio · Axum · SQLx · PostgreSQL · reqwest · Serde · thiserror · HMAC-SHA256 · GitHub Actions