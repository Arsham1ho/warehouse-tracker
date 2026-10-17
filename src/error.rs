use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("item not found")]
    NotFound,
    #[error("adjustment would make quantity negative")]
    InsufficientStock,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::InsufficientStock => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
            AppError::Db(e) => {
                // A unique-violation from Postgres is a client error, not a 500.
                if let sqlx::Error::Database(db) = e {
                    if db.code().as_deref() == Some("23505") {
                        return (
                            StatusCode::CONFLICT,
                            Json(serde_json::json!({"error":"sku already exists"})),
                        )
                            .into_response();
                    }
                }
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_string(),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}
