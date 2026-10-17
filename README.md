# warehouse-tracker

A small warehouse item-tracking service in Rust. Items have a location; the
location changes either through the REST API or through barcode-scanner events
arriving over MQTT. Every move is written to an append-only `movements` table,
so stock history is auditable rather than overwritten.

Built to learn the Rust + axum + sqlx stack on a realistic problem rather than a
toy one.

## Stack

| | |
|---|---|
| HTTP | `axum` |
| Database | PostgreSQL via `sqlx` (migrations, transactions) |
| Messaging | MQTT via `rumqttc` |
| Runtime | `tokio` |

## Running it

```sh
cp .env.example .env
docker compose up -d        # Postgres + Mosquitto
cargo run                   # migrations run on startup
```

## API

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | liveness |
| `GET` | `/items` | list items |
| `POST` | `/items` | create an item |
| `GET` | `/items/:id` | fetch one item |
| `POST` | `/items/:id/move` | move an item, body `{"to_location":"B-07"}` |
| `GET` | `/items/:id/movements` | audit trail for an item |

```sh
curl -X POST localhost:3000/items \
  -H 'content-type: application/json' \
  -d '{"sku":"ABC-1","name":"Bearing","location":"A-01","quantity":10}'

curl -X POST localhost:3000/items/<id>/move \
  -H 'content-type: application/json' \
  -d '{"to_location":"B-07"}'
```

## Scanner events

A handheld scanner publishes JSON to `warehouse/scans`:

```sh
mosquitto_pub -t warehouse/scans -m '{"sku":"ABC-1","location":"C-12"}'
```

The subscriber applies the move with `source = 'scanner'`. A scan for an unknown
SKU is logged and skipped rather than killing the loop — one bad barcode must not
stop the warehouse feed.

## Design notes

**Moves are transactional.** Updating `items.location` and appending to
`movements` happen in one transaction with `SELECT ... FOR UPDATE` on the item
row, so the current location and its audit trail can never disagree, and two
concurrent scans of the same item serialise instead of racing.

**Constraints live in the database.** `sku` is unique and `quantity >= 0` is a
CHECK constraint, so the invariants hold regardless of which path writes the row
— API, scanner, or a future importer.

**Postgres errors map to the right status codes.** A `23505` unique violation
returns `409 Conflict` rather than a generic `500`.

## Tests

```sh
docker compose up -d db
cargo test
```

Integration tests run against a real Postgres instance, because what is worth
testing here — the move-and-record transaction and the database constraints — is
database behaviour. Mocking it would only test the mock.
