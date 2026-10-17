mod error;
mod models;
mod mqtt;
mod repo;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use error::AppError;
use models::{Item, MoveItem, Movement, NewItem};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("warehouse_tracker=info".parse().unwrap()))
        .init();

    let db_url = std::env::var("DATABASE_URL")?;
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    let pool = PgPoolOptions::new().max_connections(5).connect(&db_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // The scanner feed runs alongside the API on the same pool.
    {
        let pool = pool.clone();
        let host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into());
        let port: u16 = std::env::var("MQTT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(1883);
        let topic = std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "warehouse/scans".into());
        tokio::spawn(async move { mqtt::run(pool, host, port, topic).await });
    }

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/items", get(list).post(create))
        .route("/items/:id", get(show))
        .route("/items/:id/move", post(move_to))
        .route("/items/:id/movements", get(movements))
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list(State(s): State<AppState>) -> Result<Json<Vec<Item>>, AppError> {
    Ok(Json(repo::list_items(&s.pool).await?))
}

async fn show(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Item>, AppError> {
    Ok(Json(repo::get_item(&s.pool, id).await?))
}

async fn create(State(s): State<AppState>, Json(body): Json<NewItem>) -> Result<Json<Item>, AppError> {
    Ok(Json(repo::create_item(&s.pool, body).await?))
}

async fn move_to(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<MoveItem>,
) -> Result<Json<Item>, AppError> {
    Ok(Json(repo::move_item(&s.pool, id, &body.to_location, "api").await?))
}

async fn movements(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Vec<Movement>>, AppError> {
    Ok(Json(repo::item_movements(&s.pool, id).await?))
}
