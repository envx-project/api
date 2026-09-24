# Building and upgrading the API

The Docker image builds with `SQLX_OFFLINE=true` and a committed lockfile.
It never connects to the deployment database during the build. Query metadata
in `.sqlx/` must be regenerated against a disposable PostgreSQL database after
query or schema changes (`cargo sqlx migrate run`, then `cargo sqlx prepare -- --all-targets`).
Do not use production credentials for these commands.

At startup the API establishes a database connection and runs embedded migrations
before accepting HTTP requests. SQLx serializes concurrent migration runners with
its PostgreSQL advisory lock and applies each migration transactionally. Startup
fails if a migration fails or a recorded checksum differs. Railway's health check
only succeeds after initialization has completed.

## Legacy invitation compatibility

Published migration `20251025060537` adds a required verifier without a backfill.
It works on an empty invitation table but fails on populated older installations.
Its original SQL and checksum remain unchanged in the migrations directory.

The startup migrator substitutes `migration-compat/20251025060537_invites_v2.sql`
only when that version is pending. SQLx still validates the original published
checksum for already-applied versions. The replay preserves invitation rows,
expires them, gives them an unusable verifier, and otherwise produces the original
migration's schema. Old signatures cannot be converted into the new verifiers;
users must create new invitations. As in the original migration, the obsolete
`author_signature` column is removed.

The replay records version, operation, affected-row count, and timestamp in
`envx_migration_compatibility` in the same transaction. Its recorded SQLx checksum
is deliberately the published migration checksum, so databases upgraded by either
path have compatible migration histories. Existing upgraded databases do not
replay this operation. This is a narrowly scoped historical compatibility shim,
not permission to suppress future checksum mismatches.

For an old populated database, start the API to upgrade; do not run plain
`cargo sqlx migrate run`, which intentionally executes the unmodified historical
migration. Fresh disposable databases can use the standard SQLx CLI for metadata
generation and test setup.
