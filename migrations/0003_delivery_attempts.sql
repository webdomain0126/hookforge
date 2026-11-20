CREATE TABLE delivery_attempts (
    id           BIGSERIAL PRIMARY KEY,
    delivery_id  BIGINT      NOT NULL REFERENCES deliveries(id),
    attempt      INTEGER     NOT NULL,
    status_code  INTEGER,
    error        TEXT,
    duration_ms  INTEGER     NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX delivery_attempts_by_delivery
    ON delivery_attempts (delivery_id);