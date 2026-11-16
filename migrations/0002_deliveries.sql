CREATE TABLE deliveries (
    id               BIGSERIAL PRIMARY KEY,
    endpoint_id      BIGINT      NOT NULL REFERENCES endpoints(id),
    payload          JSONB       NOT NULL,
    status           TEXT        NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'delivering', 'delivered', 'dead')),
    attempts         INTEGER     NOT NULL DEFAULT 0,
    next_attempt_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX deliveries_ready
    ON deliveries (next_attempt_at)
    WHERE status = 'pending';