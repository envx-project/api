-- Add migration script here
CREATE TABLE project_invites (
    "id" UUID NOT NULL DEfAULT gen_random_uuid(),
    "project_id" UUID NOT NULL,
    "author_id" UUID NOT NULL,
    "author_signature" TEXT NOT NULL,
    "invited_id" UUID DEFAULT NULL,
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "expires_at" TIMESTAMPTZ NOT NULL,
    PRIMARY KEY ("id"),
    CONSTRAINT "project_invites_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "projects"("id") ON DELETE CASCADE,
    CONSTRAINT "project_invites_author_id_fkey" FOREIGN KEY ("author_id") REFERENCES "users"("id") ON DELETE CASCADE,
    CONSTRAINT "project_invites_invited_id_fkey" FOREIGN KEY ("invited_id") REFERENCES "users"("id") ON DELETE CASCADE
);
