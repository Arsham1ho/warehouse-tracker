use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Item {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub location: String,
    pub quantity: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NewItem {
    pub sku: String,
    pub name: String,
    pub location: String,
    #[serde(default)]
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct MoveItem {
    pub to_location: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Movement {
    pub id: i64,
    pub item_id: Uuid,
    pub from_location: Option<String>,
    pub to_location: String,
    pub source: String,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AdjustQuantity {
    pub delta: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Adjustment {
    pub id: i64,
    pub item_id: Uuid,
    pub delta: i32,
    pub reason: String,
    pub quantity_after: i32,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub location: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A scan event as published by a handheld scanner onto MQTT.
#[derive(Debug, Deserialize)]
pub struct ScanEvent {
    pub sku: String,
    pub location: String,
}
