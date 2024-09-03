-- Add migration script here

BEGIN;

  ALTER TABLE "public"."variables" ALTER COLUMN "tag" SET DEFAULT '';

COMMIT;
