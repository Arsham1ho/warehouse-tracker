CREATE TABLE IF NOT EXISTS items (
    id          UUID PRIMARY KEY,
    sku         TEXT        NOT NULL UNIQUE,
    name        TEXT        NOT NULL,
    location    TEXT        NOT NULL,
    quantity    INTEGER     NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every location change is appended here, so stock movement is auditable
-- rather than overwritten. Scanner events and API calls both land in this table.
CREATE TABLE IF NOT EXISTS movements (
    id            BIGSERIAL PRIMARY KEY,
    item_id       UUID        NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    from_location TEXT,
    to_location   TEXT        NOT NULL,
    source        TEXT        NOT NULL,          -- 'api' | 'scanner'
    occurred_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS movements_item_idx ON movements(item_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS items_location_idx ON items(location);
