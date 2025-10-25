-- Add migration script here
ALTER TABLE project_invites
    ADD COLUMN "verifier_argon2id" TEXT NOT NULL;

ALTER TABLE project_invites
    ADD COLUMN "ciphertext" TEXT;

ALTER TABLE project_invites
    DROP COLUMN "author_signature";
