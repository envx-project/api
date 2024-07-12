-- Migration Script to Add "tag" Column to "variables" Table

-- Begin a transaction
BEGIN;

-- Add the "tag" column
ALTER TABLE "public"."variables"
ADD COLUMN "tag" varchar(24);

-- Commit the transaction
COMMIT;
