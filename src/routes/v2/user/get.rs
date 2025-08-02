use super::*;
use super::{structs::StrippedUser, AppError, AppState, UserId};
use axum::{
    extract::{Path, State},
    Json,
};

#[utoipa::path(
    get,
    path = "/{user_id}",
    tag = USER_TAG,
    responses(
        (status = 200, description = "User", body = StrippedUser),
        (status = 404, description = "User not found"),
    )
)]
pub async fn get_user_v2(
    State(state): State<AppState>,
    _: UserId, // check if the user is authenticated
    Path(user_id): Path<Uuid>,
) -> Result<Json<StrippedUser>, AppError> {
    let user = sqlx::query_as!(
        StrippedUser,
        "SELECT id, public_key FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get user")?;

    Ok(Json(user))
}
