use super::*;
use super::{structs::StrippedUser, AppError, AppState, UserId};
use axum::{extract::State, Json};

#[utoipa::path(
    get,
    path = "/get-many",
    tag = USER_TAG,
    responses(
        (status = 200, description = "Users", body = Vec<StrippedUser>),
    )
)]
pub async fn get_many_users(
    State(state): State<AppState>,
    _: UserId, // just want to check if the user is authenticated
    Json(body): Json<Vec<Uuid>>,
) -> Result<Json<Vec<StrippedUser>>, AppError> {
    let users = sqlx::query_as!(
        StrippedUser,
        "SELECT id, public_key FROM users WHERE id = ANY($1)",
        &body
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get users")?;

    Ok(Json(users))
}
