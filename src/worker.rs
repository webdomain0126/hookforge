use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;

use crate::signature;

/// One claimed delivery, ready to send.
struct Job {
    id: i64,
    url: String,
    secret: String,
    payload: Value,
}

/// Runs forever: claim one pending delivery, send it, repeat.
pub async fn run(pool: PgPool) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("could not build HTTP client");

    println!("worker started");

    loop {
        match claim_one(&pool).await {
            Ok(Some(job)) => deliver(&pool, &client, job).await,
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
    let row: Option<(i64, String, String, Value)> = sqlx::query_as(
        "WITH next AS (
             SELECT id FROM deliveries
             WHERE status = 'pending' AND next_attempt_at <= now()
             ORDER BY next_attempt_at
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         UPDATE deliveries d
         SET status = 'delivering', attempts = d.attempts + 1
         FROM next, endpoints e
         WHERE d.id = next.id AND e.id = d.endpoint_id
         RETURNING d.id, e.url, e.secret, d.payload",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, url, secret, payload)| Job { id, url, secret, payload }))
}

/// Signs the payload, sends it, and records the result.
async fn deliver(pool: &PgPool, client: &reqwest::Client, job: Job) {
    let body = serde_json::to_vec(&job.payload).expect("JSON is always serializable");
    let timestamp = chrono::Utc::now().timestamp();
    let sig = signature::sign(&job.secret, timestamp, &body);

    let ok = match client
        .post(&job.url)
        .header("Content-Type", "application/json")
        .header("X-HookForge-Timestamp", timestamp.to_string())
        .header("X-HookForge-Signature", format!("v1={sig}"))
        .body(body)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };

    let sql = if ok {
        "UPDATE deliveries SET status = 'delivered' WHERE id = $1"
    } else {
        // Simple retry for now: try again in 30 seconds. M6 makes this smarter.
        "UPDATE deliveries
         SET status = 'pending', next_attempt_at = now() + interval '30 seconds'
         WHERE id = $1"
    };

    if let Err(e) = sqlx::query(sql).bind(job.id).execute(pool).await {
        eprintln!("worker: could not update delivery {}: {e}", job.id);
    }

    println!("delivery {} -> {}", job.id, if ok { "delivered" } else { "failed, will retry" });
}