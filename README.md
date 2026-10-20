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
| `GET` | `/health` | readiness — also checks the database |
| `GET` | `/items` | list items; `?location=A-01&limit=50&offset=0` |
| `POST` | `/items` | create an item |
| `GET` | `/items/:id` | fetch one item |
| `DELETE` | `/items/:id` | delete an item and its history |
| `POST` | `/items/:id/move` | move an item, body `{"to_location":"B-07"}` |
| `GET` | `/items/:id/movements` | movement history |
| `POST` | `/items/:id/adjust` | change stock, body `{"delta":-3,"reason":"damaged"}` |
| `GET` | `/items/:id/adjustments` | stock-change history |

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
returns `409 Conflict` rather than a generic `500`, and an adjustment that would
take stock below zero returns `422` — caught in the handler so the caller gets a
clear reason instead of a constraint error surfacing as a `500`.

**Quantity changes are audited like moves.** Adjustments append to their own
table with a reason and the resulting quantity, under the same row lock, so two
concurrent adjustments cannot both read the same starting quantity and lose one
of the writes.

**Configuration is validated at startup.** A missing or malformed variable fails
immediately with a clear message rather than surfacing on the first request.

## Tests

```sh
docker compose up -d db
cargo test
```

CI runs `cargo fmt --check`, `cargo clippy -D warnings` and the full test suite
against a real Postgres service container on every push. The same pipeline is
mirrored in `.gitlab-ci.yml` for GitLab CI.

## AI review gate

AI assistants are useful and also wrong in ways ordinary tools are not: they
can read as confident while being wrong, answer the same question differently
twice, and lose detail in a long file. So using one is fine; letting its output
reach `main` unchecked is not.

`tools/ai_review_gate.py` enforces three rules on every pull request:

1. **Declare it** — a commit written with AI assistance carries `AI-Assisted: <tool>`
2. **Review it** — that commit must also carry `Reviewed-by: <name>`
3. **Record it** — the approved change is appended to `docs/ai-assistance-log.md`

A declared commit without a reviewer, or with a reviewer but no log entry, fails
the build. The point is not to discourage the tooling; it is that a human took
responsibility for the output, and that the fact stays auditable after the pull
request is closed.

Integration tests run against a real Postgres instance, because what is worth
testing here — the move-and-record transaction and the database constraints — is
database behaviour. Mocking it would only test the mock.
