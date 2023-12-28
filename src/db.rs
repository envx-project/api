use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;

pub async fn db() -> Result<sqlx::Pool<sqlx::Postgres>> {
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?)
}
