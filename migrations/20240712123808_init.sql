-- Add migration script here

CREATE TABLE IF NOT EXISTS "public"."projects" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    PRIMARY KEY ("id")
);

CREATE TABLE IF NOT EXISTS "public"."users" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "username" varchar NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "public_key" text NOT NULL,
    PRIMARY KEY ("id")
);

CREATE TABLE IF NOT EXISTS "public"."user_project_relations" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL,
    "project_id" uuid NOT NULL,
    CONSTRAINT "user_project_relations_user_id_fkey"    FOREIGN KEY ("user_id")    REFERENCES "public"."users"("id")    ON DELETE CASCADE,
    CONSTRAINT "user_project_relations_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS "public"."variables" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "value" text NOT NULL,
    "project_id" uuid NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "variables_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id")
);
