CREATE TABLE endpoints (
    id              BIGSERIAL PRIMARY KEY,
    url             TEXT        NOT NULL,
    secret          TEXT        NOT NULL,
    max_per_second  INTEGER     NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);