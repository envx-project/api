use crate::extractors::user::UserId;
use sqlx::types::Uuid;

use crate::*;

pub async fn test_auth(State(state): State<AppState>, user_id: UserId) -> Result<String, AppError> {
    let uuid_id = Uuid::parse_str(&user_id.to_string())?;

    let user = sqlx::query!("SELECT id, username FROM users WHERE id = $1", uuid_id)
        .fetch_one(&*state.db)
        .await
        .context("Failed to get user")?;

    Ok(format!("user: {:?}", user))
}
