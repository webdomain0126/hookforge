use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::watch;

use crate::{ratelimit::RateLimiter, retry, signature};

/// One claimed delivery, ready to send.
struct Job {
    id: i64,
    attempts: i32,
    endpoint_id: i64,
    url: String,
    secret: String,
    max_per_second: i32,
    payload: Value,
}

/// Runs until shutdown: claim one pending delivery, send it, repeat.
pub async fn run(
    worker_id: u32,
    pool: PgPool,
    limiter: Arc<RateLimiter>,
    shutdown: watch::Receiver<bool>,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("could not build HTTP client");

    println!("worker {worker_id} started");

    loop {
        if *shutdown.borrow() {
            println!("worker {worker_id} stopped");
            return;
        }

        match claim_one(&pool).await {
            Ok(Some(job)) => {
                // Too many sends to this endpoint this second? Put it back for later.
                if !limiter.try_acquire(job.endpoint_id, job.max_per_second as u32) {
                    release(&pool, job.id).await;
                    println!("delivery {} -> rate limited, retry in 1s", job.id);
                    continue;
                }

                println!("worker {worker_id} claimed delivery {}", job.id);
                deliver(&pool, &client, job).await;
            }
            Ok(None) => tokio::time::sleep(Duration::from_secs(1)).await,
            Err(e) => {
                eprintln!("worker: claim failed: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Finds one due delivery and marks it 'delivering' so no other worker takes it.
async fn claim_one(pool: &PgPool) -> Result<Option<Job>, sqlx::Error> {
    let row: Option<(i64, i32, i64, String, String, i32, Value)> = sqlx::query_as(
        "WITH next AS (
             SELECT id FROM deliveries
             WHERE status = 'pending' AND next_attempt_at <= now()
             ORDER BY next_attempt_at
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         UPDATE deliveries d
         SET status = 'delivering', attempts = d.attempts + 1,
             locked_until = now() + interval '60 seconds'
         FROM next, endpoints e
         WHERE d.id = next.id AND e.id = d.endpoint_id
         RETURNING d.id, d.attempts, d.endpoint_id, e.url, e.secret, e.max_per_second, d.payload",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, attempts, endpoint_id, url, secret, max_per_second, payload)| Job {
            id,
            attempts,
            endpoint_id,
            url,
            secret,
            max_per_second,
            payload,
        },
    ))
}

/// Gives a claimed delivery back without counting it as an attempt.
async fn release(pool: &PgPool, id: i64) {
    if let Err(e) = sqlx::query(
        "UPDATE deliveries
         SET status = 'pending', attempts = attempts - 1, locked_until = NULL,
             next_attempt_at = now() + interval '1 second'
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    {
        eprintln!("worker: could not release delivery {id}: {e}");
    }
}

/// Signs and sends the payload, logs the attempt, and decides what happens next.
async fn deliver(pool: &PgPool, client: &reqwest::Client, job: Job) {
    let body = serde_json::to_vec(&job.payload).expect("JSON is always serializable");
    let timestamp = chrono::Utc::now().timestamp();
    let sig = signature::sign(&job.secret, timestamp, &body);

    // 1. Send it, and time how long it takes.
    let started = Instant::now();
    let result = client
        .post(&job.url)
        .header("Content-Type", "application/json")
        .header("X-HookForge-Timestamp", timestamp.to_string())
        .header("X-HookForge-Signature", format!("v1={sig}"))
        .body(body)
        .send()
        .await;
    let duration_ms = started.elapsed().as_millis() as i32;

    // 2. Work out what happened.
    let (ok, status_code, error): (bool, Option<i32>, Option<String>) = match result {
        Ok(resp) => {
            let code = resp.status().as_u16() as i32;
            let ok = resp.status().is_success();
            let error = if ok { None } else { Some(format!("HTTP {code}")) };
            (ok, Some(code), error)
        }
        Err(e) => (false, None, Some(e.to_string())),
    };

    // 3. Write it in the attempt log.
    if let Err(e) = sqlx::query(
        "INSERT INTO delivery_attempts (delivery_id, attempt, status_code, error, duration_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(job.id)
    .bind(job.attempts)
    .bind(status_code)
    .bind(&error)
    .bind(duration_ms)
    .execute(pool)
    .await
    {
        eprintln!("worker: could not log attempt for delivery {}: {e}", job.id);
    }

    // 4. Delivered, dead, or retry later?
    let (outcome, update) = if ok {
        (
            "delivered".to_string(),
            sqlx::query("UPDATE deliveries SET status = 'delivered' WHERE id = $1").bind(job.id),
        )
    } else if retry::after_failure(job.attempts).is_terminal() {
        (
            format!("dead after {} attempts", job.attempts),
            sqlx::query("UPDATE deliveries SET status = 'dead' WHERE id = $1").bind(job.id),
        )
    } else {
        let wait = retry::backoff(job.attempts);
        (
            format!("failed, retry in {}s", wait.as_secs()),
            sqlx::query(
                "UPDATE deliveries
                 SET status = 'pending', next_attempt_at = now() + make_interval(secs => $2)
                 WHERE id = $1",
            )
            .bind(job.id)
            .bind(wait.as_secs() as f64),
        )
    };

    if let Err(e) = update.execute(pool).await {
        eprintln!("worker: could not update delivery {}: {e}", job.id);
    }

    println!("delivery {} -> {outcome}", job.id);
}