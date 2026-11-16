use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;

/// Runs forever: claim one pending delivery, send it, repeat.
pub async fn run(pool: PgPool) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("could not build HTTP client");

    println!("worker started");

    loop {
        match claim_one(&pool).await {
            Ok(Some((id, url, payload))) => {
                deliver(&pool, &client, id, &url, &payload).await;
            }
            Ok(None) => {
                // Nothing to do. Wait a second and look again.
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                eprintln!("worker: claim failed: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Finds one due delivery and marks it 'delivering' so no other worker takes it.
async fn claim_one(pool: &PgPool) -> Result<Option<(i64, String, Value)>, sqlx::Error> {
    sqlx::query_as(
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
         RETURNING d.id, e.url, d.payload",
    )
    .fetch_optional(pool)
    .await
}

/// Sends the payload and records the result.
async fn deliver(pool: &PgPool, client: &reqwest::Client, id: i64, url: &str, payload: &Value) {
    let ok = match client.post(url).json(payload).send().await {
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

    if let Err(e) = sqlx::query(sql).bind(id).execute(pool).await {
        eprintln!("worker: could not update delivery {id}: {e}");
    }

    println!("delivery {id} -> {}", if ok { "delivered" } else { "failed, will retry" });
}