use crate::error::AppError;
use crate::models::{Adjustment, Item, ListQuery, Movement, NewItem};
use sqlx::PgPool;
use uuid::Uuid;

/// Lists items, optionally filtered by location. Paging is capped so a client
/// cannot ask for the whole warehouse in one request.
pub async fn list_items(pool: &PgPool, q: &ListQuery) -> Result<Vec<Item>, AppError> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);

    let items = sqlx::query_as::<_, Item>(
        "SELECT id, sku, name, location, quantity, updated_at
         FROM items
         WHERE ($1::text IS NULL OR location = $1)
         ORDER BY sku
         LIMIT $2 OFFSET $3",
    )
    .bind(q.location.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn delete_item(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let rows = sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if rows == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Applies a stock delta and records why. Runs in a transaction with the item
/// row locked, so two concurrent adjustments cannot both read the same starting
/// quantity and lose one of the writes.
pub async fn adjust_quantity(
    pool: &PgPool,
    id: Uuid,
    delta: i32,
    reason: &str,
) -> Result<Item, AppError> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, Item>(
        "SELECT id, sku, name, location, quantity, updated_at FROM items WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_quantity = current.quantity + delta;
    if new_quantity < 0 {
        // Checked here so the caller gets a clear 422 rather than a database
        // constraint error surfacing as a 500.
        return Err(AppError::InsufficientStock);
    }

    let updated = sqlx::query_as::<_, Item>(
        "UPDATE items SET quantity = $2, updated_at = now() WHERE id = $1
         RETURNING id, sku, name, location, quantity, updated_at",
    )
    .bind(id)
    .bind(new_quantity)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO adjustments (item_id, delta, reason, quantity_after)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(delta)
    .bind(reason)
    .bind(new_quantity)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}

pub async fn item_adjustments(pool: &PgPool, id: Uuid) -> Result<Vec<Adjustment>, AppError> {
    let rows = sqlx::query_as::<_, Adjustment>(
        "SELECT id, item_id, delta, reason, quantity_after, occurred_at
         FROM adjustments WHERE item_id = $1 ORDER BY occurred_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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
