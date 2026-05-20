use crate::config::Caps;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DB,
    pub caps: Arc<Caps>,
}

pub type DB = Arc<sqlx::Pool<sqlx::Postgres>>;
