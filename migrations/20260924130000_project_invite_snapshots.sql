-- Existing invitations have no verifiable source snapshot and must be regenerated.
-- Preserve their ciphertext and all existing memberships for explicit safe failure.
ALTER TABLE project_invites ADD COLUMN snapshot_hash TEXT;
CREATE INDEX project_invites_author_live_payload ON project_invites(author_id, expires_at)
WHERE ciphertext IS NOT NULL;
