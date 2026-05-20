-- Track per-IP write volume so we can rate-limit by IP.
-- Pruned opportunistically: rows older than 24h are deleted as part of every
-- write that reads from this table.
CREATE TABLE IF NOT EXISTS "public"."upload_log" (
    "id"         uuid        NOT NULL DEFAULT gen_random_uuid(),
    "ip"         inet        NOT NULL,
    "bytes"      bigint      NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("id")
);

CREATE INDEX IF NOT EXISTS upload_log_ip_created_at_idx
    ON upload_log (ip, created_at DESC);
