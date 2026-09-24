use anyhow::{Context, Result};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};

pub async fn db() -> Result<PgPool> {
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

fn startup_migrator() -> Migrator {
    let mut migrator = sqlx::migrate!();
    // The published migration fails on populated legacy databases. Keep its
    // checksum unchanged so already-upgraded databases still validate, but use
    // a transactionally audited compatibility replay when it is still pending.
    // SQLx continues to reject every mismatched applied checksum.
    for migration in migrator.migrations.to_mut() {
        if migration.version == 20251025060537 {
            migration.sql =
                include_str!("../migration-compat/20251025060537_invites_v2.sql").into();
        }
    }
    migrator
}

async fn migrate(pool: &PgPool) -> Result<()> {
    startup_migrator()
        .run(pool)
        .await
        .context("database migrations failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use uuid::Uuid;

    async fn legacy_invite(pool: &PgPool) -> Uuid {
        let mut legacy = sqlx::migrate!();
        legacy.migrations = Cow::Owned(
            legacy
                .iter()
                .filter(|m| m.version < 20251025060537)
                .cloned()
                .collect(),
        );
        legacy.run(pool).await.unwrap();
        let project: Uuid = sqlx::query_scalar("INSERT INTO projects DEFAULT VALUES RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
        let user: Uuid = sqlx::query_scalar(
            "INSERT INTO users(username,public_key) VALUES('legacy','fixture') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query_scalar("INSERT INTO project_invites(project_id,author_id,author_signature,expires_at) VALUES($1,$2,'legacy-signature',CURRENT_TIMESTAMP + INTERVAL '1 day') RETURNING id")
            .bind(project).bind(user).fetch_one(pool).await.unwrap()
    }

    #[sqlx::test(migrations = false)]
    async fn startup_migrates_an_empty_database_and_is_repeatable(pool: PgPool) {
        migrate(&pool).await.unwrap();
        migrate(&pool).await.unwrap();
        let table: Option<String> =
            sqlx::query_scalar("SELECT to_regclass('public.upload_log')::text")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(table.as_deref(), Some("upload_log"));
    }

    #[sqlx::test(migrations = false)]
    async fn startup_preserves_but_expires_legacy_invites(pool: PgPool) {
        let id = legacy_invite(&pool).await;
        migrate(&pool).await.unwrap();
        migrate(&pool).await.unwrap();
        let (expired, verifier): (bool, String) = sqlx::query_as("SELECT expires_at <= CURRENT_TIMESTAMP, verifier_argon2id FROM project_invites WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert!(expired);
        assert_eq!(verifier, "legacy-invite-unusable");
        let affected: i64 = sqlx::query_scalar("SELECT affected_rows FROM envx_migration_compatibility WHERE migration_version=20251025060537")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(affected, 1);
        let recorded: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version=20251025060537",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let original = sqlx::migrate!();
        assert_eq!(
            recorded,
            original
                .iter()
                .find(|m| m.version == 20251025060537)
                .unwrap()
                .checksum
                .as_ref()
        );
    }

    #[sqlx::test]
    async fn startup_accepts_original_migration_checksums(pool: PgPool) {
        migrate(&pool).await.unwrap();
        let audit: Option<String> =
            sqlx::query_scalar("SELECT to_regclass('public.envx_migration_compatibility')::text")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            audit.is_none(),
            "applied original migrations must not be replayed"
        );
    }

    #[sqlx::test(migrations = false)]
    async fn failed_compatibility_replay_rolls_back_rows_and_schema(pool: PgPool) {
        let id = legacy_invite(&pool).await;
        let mut broken = startup_migrator();
        let migration = broken
            .migrations
            .to_mut()
            .iter_mut()
            .find(|m| m.version == 20251025060537)
            .unwrap();
        migration.sql = format!("{}\nSELECT 1 / 0;", migration.sql).into();
        assert!(broken.run(&pool).await.is_err());
        let (signature, valid): (String, bool) = sqlx::query_as("SELECT author_signature, expires_at > CURRENT_TIMESTAMP FROM project_invites WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(signature, "legacy-signature");
        assert!(valid);
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=20251025060537)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!applied);
        // Reopening after an interrupted transaction can safely finish migration.
        migrate(&pool).await.unwrap();
    }

    #[sqlx::test]
    async fn startup_rejects_changed_historical_checksums(pool: PgPool) {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = decode('00','hex') WHERE version=20251025060537")
            .execute(&pool).await.unwrap();
        let error = migrate(&pool).await.unwrap_err();
        assert!(format!("{error:#}").contains("modified"));
    }
}
