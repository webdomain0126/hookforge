use std::time::Duration;

use sqlx::PgPool;

/// Every 10 seconds, puts stuck 'delivering' messages back to 'pending'.
pub async fn run(pool: PgPool) {
    println!("reaper started");

    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let result = sqlx::query(
            "UPDATE deliveries
             SET status = 'pending', locked_until = NULL
             WHERE status = 'delivering'
               AND (locked_until IS NULL OR locked_until < now())",
        )
        .execute(&pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                println!("reaper: recovered {} stuck deliveries", r.rows_affected());
            }
            Ok(_) => {}
            Err(e) => eprintln!("reaper: {e}"),
        }
    }
}