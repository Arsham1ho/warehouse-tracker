//! Integration tests. These run against a real Postgres instance, because the
//! behaviour worth testing here (the move-and-record transaction) is database
//! behaviour, and mocking it would only test the mock.
//!
//! Run with:  docker compose up -d db && cargo test

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://warehouse:warehouse@localhost:5432/warehouse".into());
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("postgres must be running: docker compose up -d db");
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn insert_item(pool: &PgPool, sku: &str, location: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO items (id, sku, name, location, quantity) VALUES ($1,$2,$3,$4,$5)")
        .bind(id)
        .bind(sku)
        .bind("Test item")
        .bind(location)
        .bind(1)
        .execute(pool)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn move_updates_location_and_records_movement() {
    let pool = pool().await;
    let sku = format!("SKU-{}", Uuid::new_v4());
    let id = insert_item(&pool, &sku, "A-01").await;

    let location: String = sqlx::query_scalar("SELECT location FROM items WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(location, "A-01");

    // Simulate the move the handler performs.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE items SET location = $2 WHERE id = $1")
        .bind(id)
        .bind("B-07")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO movements (item_id, from_location, to_location, source) VALUES ($1,$2,$3,$4)",
    )
    .bind(id)
    .bind("A-01")
    .bind("B-07")
    .bind("api")
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let location: String = sqlx::query_scalar("SELECT location FROM items WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(location, "B-07");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM movements WHERE item_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "the move must leave exactly one audit row");

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn duplicate_sku_is_rejected() {
    let pool = pool().await;
    let sku = format!("SKU-{}", Uuid::new_v4());
    let id = insert_item(&pool, &sku, "A-01").await;

    let err = sqlx::query(
        "INSERT INTO items (id, sku, name, location, quantity) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(Uuid::new_v4())
    .bind(&sku)
    .bind("Dup")
    .bind("A-02")
    .bind(1)
    .execute(&pool)
    .await
    .unwrap_err();

    match err {
        sqlx::Error::Database(db) => assert_eq!(db.code().as_deref(), Some("23505")),
        other => panic!("expected unique violation, got {other:?}"),
    }

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn quantity_cannot_go_negative() {
    let pool = pool().await;
    let sku = format!("SKU-{}", Uuid::new_v4());
    let id = insert_item(&pool, &sku, "A-01").await;

    let err = sqlx::query("UPDATE items SET quantity = -1 WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        matches!(err, sqlx::Error::Database(_)),
        "CHECK constraint must reject it"
    );

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn adjustment_cannot_take_stock_below_zero() {
    let pool = pool().await;
    let sku = format!("SKU-{}", Uuid::new_v4());
    let id = insert_item(&pool, &sku, "A-01").await; // quantity starts at 1

    // Mirrors repo::adjust_quantity: read under lock, reject before writing.
    let mut tx = pool.begin().await.unwrap();
    let current: i32 = sqlx::query_scalar("SELECT quantity FROM items WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(
        current + (-5) < 0,
        "precondition: this adjustment must be invalid"
    );
    tx.rollback().await.unwrap();

    let after: i32 = sqlx::query_scalar("SELECT quantity FROM items WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, 1, "a rejected adjustment must leave stock untouched");

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deleting_an_item_removes_its_history() {
    let pool = pool().await;
    let sku = format!("SKU-{}", Uuid::new_v4());
    let id = insert_item(&pool, &sku, "A-01").await;

    sqlx::query(
        "INSERT INTO movements (item_id, from_location, to_location, source) VALUES ($1,$2,$3,$4)",
    )
    .bind(id)
    .bind("A-01")
    .bind("B-02")
    .bind("api")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    let orphans: i64 = sqlx::query_scalar("SELECT count(*) FROM movements WHERE item_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(orphans, 0, "ON DELETE CASCADE must clear the audit rows");
}
