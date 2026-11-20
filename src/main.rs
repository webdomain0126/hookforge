mod delivery;
mod endpoint;
mod worker;
mod signature;
mod retry;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};

use endpoint::NewEndpoint;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("could not connect to PostgreSQL");
        tokio::spawn(worker::run(pool.clone()));
    let app = Router::new()
        .route("/health", get(health))
        .route("/endpoints", post(create_endpoint))
        .route("/events", post(create_event))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("could not use port 3000");

    println!("HookForge listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.expect("server error");
}

async fn health() -> &'static str {
    "ok"
}

type ApiResult = Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)>;

fn api_error(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": message })))
}

async fn create_endpoint(
    State(pool): State<PgPool>,
    Json(new): Json<NewEndpoint>,
) -> ApiResult {
    if let Err(e) = new.validate() {
        return Err(api_error(StatusCode::BAD_REQUEST, &e.to_string()));
    }

    let row: (i64,) = sqlx::query_as(
        "INSERT INTO endpoints (url, secret, max_per_second)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(&new.url)
    .bind(&new.secret)
    .bind(new.max_per_second as i32)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("database error: {e}");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "id": row.0 }))))
}

/// What an app sends when it wants a message delivered.
#[derive(Deserialize)]
struct NewEvent {
    endpoint_id: i64,
    payload: Value,
}

async fn create_event(
    State(pool): State<PgPool>,
    Json(event): Json<NewEvent>,
) -> ApiResult {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO deliveries (endpoint_id, payload)
         VALUES ($1, $2)
         RETURNING id",
    )
    .bind(event.endpoint_id)
    .bind(&event.payload)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        // The endpoint id does not exist in the endpoints table.
        let missing_endpoint = e
            .as_database_error()
            .map(|d| d.is_foreign_key_violation())
            .unwrap_or(false);

        if missing_endpoint {
            api_error(StatusCode::NOT_FOUND, "endpoint not found")
        } else {
            eprintln!("database error: {e}");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    })?;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "delivery_id": row.0, "status": "pending" })),
    ))
}