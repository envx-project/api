use super::*;
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/{project_id}",
    tag = PROJECT_TAG,
    security(
        ("bearer" = []),
    ),
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
        (status = 401, description = "Unauthorized. User not in project"),
    )
)]
pub async fn add_user(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<sqlx::types::Uuid>,
    Json(users_to_add): Json<Vec<Uuid>>,
) -> Result<(), AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id)
        SELECT user_id, $2::uuid
        FROM UNNEST($1::uuid[]) AS t(user_id)
        ON CONFLICT DO NOTHING",
        &users_to_add,
        project_id,
    )
    .execute(&*state.db)
    .await
    .context("Failed to insert user project relations")?;

    Ok(())
}
