-- Add migration script here

-- This script only contains the table creation statements and does not fully represent the table in the database. It's still missing: indices, triggers. Do not use it as a backup.

-- Table Definition
CREATE TABLE "public"."projects" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    PRIMARY KEY ("id")
);
CREATE 
