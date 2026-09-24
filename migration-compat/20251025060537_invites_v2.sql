-- Compatibility replay for the original pending migration, which cannot add a
-- NOT NULL verifier to a populated legacy table. Legacy signatures cannot be
-- converted into Argon2 verifiers, so retain those invitations as expired rows.
ALTER TABLE project_invites
    ADD COLUMN verifier_argon2id TEXT NOT NULL DEFAULT 'legacy-invite-unusable';

CREATE TABLE IF NOT EXISTS envx_migration_compatibility (
    migration_version BIGINT PRIMARY KEY,
    operation TEXT NOT NULL,
    affected_rows BIGINT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

WITH expired AS (
    UPDATE project_invites
    SET expires_at = LEAST(expires_at, CURRENT_TIMESTAMP)
    RETURNING id
)
INSERT INTO envx_migration_compatibility (migration_version, operation, affected_rows)
SELECT 20251025060537, 'retain-and-expire-legacy-invites-v1', COUNT(*) FROM expired;

ALTER TABLE project_invites
    ALTER COLUMN verifier_argon2id DROP DEFAULT;
ALTER TABLE project_invites
    ADD COLUMN ciphertext TEXT;
ALTER TABLE project_invites
    DROP COLUMN author_signature;
