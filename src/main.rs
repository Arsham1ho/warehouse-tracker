mod config;
mod error;
mod models;
mod mqtt;
mod repo;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use error::AppError;
use config::Config;
use models::{AdjustQuantity, Adjustment, Item, ListQuery, MoveItem, Movement, NewItem};
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

    let cfg = Config::from_env()?;

    let pool = PgPoolOptions::new().max_connections(5).connect(&cfg.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // The scanner feed runs alongside the API on the same pool.
    {
        let pool = pool.clone();
        let c = cfg.clone();
        tokio::spawn(async move { mqtt::run(pool, c.mqtt_host, c.mqtt_port, c.mqtt_topic).await });
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/items", get(list).post(create))
        .route("/items/:id", get(show).delete(remove))
        .route("/items/:id/move", post(move_to))
        .route("/items/:id/movements", get(movements))
        .route("/items/:id/adjust", post(adjust))
        .route("/items/:id/adjustments", get(adjustments))
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!(addr = %cfg.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Readiness, not just liveness: an instance that cannot reach Postgres should
/// not report itself healthy.
async fn health(State(s): State<AppState>) -> Result<&'static str, AppError> {
    sqlx::query("SELECT 1").execute(&s.pool).await?;
    Ok("ok")
}

async fn list(
    State(s): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<Item>>, AppError> {
    Ok(Json(repo::list_items(&s.pool, &q).await?))
}

async fn remove(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    repo::delete_item(&s.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn adjust(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AdjustQuantity>,
) -> Result<Json<Item>, AppError> {
    Ok(Json(repo::adjust_quantity(&s.pool, id, body.delta, &body.reason).await?))
}

async fn adjustments(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Adjustment>>, AppError> {
    Ok(Json(repo::item_adjustments(&s.pool, id).await?))
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
