use crate::extractors::user::UserId;
use crate::*;

pub async fn test_auth(
    State(state): State<AppState>,
    UserId(user_id): UserId,
) -> Result<String, AppError> {
    let user = sqlx::query!("SELECT id, username FROM users WHERE id = $1", &user_id)
        .fetch_one(&*state.db)
        .await
        .context("Failed to get user")?;

    Ok(format!("user: {:?}", user))
}
