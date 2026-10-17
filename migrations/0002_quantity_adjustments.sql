-- Quantity changes get the same treatment as location changes: append-only,
-- with the reason recorded, so stock discrepancies can be traced to a cause.
CREATE TABLE IF NOT EXISTS adjustments (
    id           BIGSERIAL   PRIMARY KEY,
    item_id      UUID        NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    delta        INTEGER     NOT NULL,
    reason       TEXT        NOT NULL,
    quantity_after INTEGER   NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS adjustments_item_idx ON adjustments(item_id, occurred_at DESC);
