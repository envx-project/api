use crate::{config::Caps, state::AppState};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
pub fn state(pool: PgPool) -> AppState {
    AppState {
        db: Arc::new(pool),
        caps: Arc::new(Caps {
            max_variable_bytes: 1024,
            max_variables_per_project: 256,
            max_project_bytes: 0,
            max_user_bytes: 0,
            max_ip_bytes_per_day: 0,
        }),
    }
}
pub async fn user(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users(username, public_key) VALUES ('test', 'fixture') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}
pub async fn project(pool: &PgPool, user: Uuid) -> Uuid {
    let id = sqlx::query_scalar("INSERT INTO projects DEFAULT VALUES RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_project_relations(user_id, project_id) VALUES ($1,$2)")
        .bind(user)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}
