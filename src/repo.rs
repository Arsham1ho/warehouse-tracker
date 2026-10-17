use crate::error::AppError;
use crate::models::{Item, Movement, NewItem};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_items(pool: &PgPool) -> Result<Vec<Item>, AppError> {
    let items = sqlx::query_as::<_, Item>(
        "SELECT id, sku, name, location, quantity, updated_at FROM items ORDER BY sku",
    )
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn get_item(pool: &PgPool, id: Uuid) -> Result<Item, AppError> {
    sqlx::query_as::<_, Item>(
        "SELECT id, sku, name, location, quantity, updated_at FROM items WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn create_item(pool: &PgPool, new: NewItem) -> Result<Item, AppError> {
    let id = Uuid::new_v4();
    let item = sqlx::query_as::<_, Item>(
        "INSERT INTO items (id, sku, name, location, quantity)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, sku, name, location, quantity, updated_at",
    )
    .bind(id)
    .bind(&new.sku)
    .bind(&new.name)
    .bind(&new.location)
    .bind(new.quantity)
    .fetch_one(pool)
    .await?;
    Ok(item)
}

/// Moves an item and records the movement in one transaction, so the item row
/// and its audit trail can never disagree.
pub async fn move_item(
    pool: &PgPool,
    id: Uuid,
    to_location: &str,
    source: &str,
) -> Result<Item, AppError> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, Item>(
        "SELECT id, sku, name, location, quantity, updated_at FROM items WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let updated = sqlx::query_as::<_, Item>(
        "UPDATE items SET location = $2, updated_at = now() WHERE id = $1
         RETURNING id, sku, name, location, quantity, updated_at",
    )
    .bind(id)
    .bind(to_location)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO movements (item_id, from_location, to_location, source)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(&current.location)
    .bind(to_location)
    .bind(source)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}

pub async fn move_item_by_sku(
    pool: &PgPool,
    sku: &str,
    to_location: &str,
    source: &str,
) -> Result<Item, AppError> {
    let id: Uuid = sqlx::query_scalar("SELECT id FROM items WHERE sku = $1")
        .bind(sku)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    move_item(pool, id, to_location, source).await
}

pub async fn item_movements(pool: &PgPool, id: Uuid) -> Result<Vec<Movement>, AppError> {
    let rows = sqlx::query_as::<_, Movement>(
        "SELECT id, item_id, from_location, to_location, source, occurred_at
         FROM movements WHERE item_id = $1 ORDER BY occurred_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
