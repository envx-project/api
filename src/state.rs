use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DB,
}

pub type DB = Arc<sqlx::Pool<sqlx::Postgres>>;
