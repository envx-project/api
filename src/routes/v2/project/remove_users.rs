use super::{traits::to_uuid::ToUuid, *};
use uuid::Uuid;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct RemoveUserBody {
    user_ids: Vec<String>,
}

#[utoipa::path(
    delete,
    path = "/{project_id}",
    tag = PROJECT_TAG,
    security(
        ("bearer" = []),
    ),
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn remove_users(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RemoveUserBody>,
) -> Result<(), AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let users_to_remove = body
        .user_ids
        .iter()
        .map(|user| user.to_uuid())
        .collect::<Result<Vec<Uuid>, _>>()?;

    sqlx::query!(
        "DELETE FROM user_project_relations WHERE user_id = ANY($1::uuid[]) AND project_id = $2",
        &users_to_remove,
        project_id
    )
    .execute(&*state.db)
    .await
    .context("Failed to remove user from project")?;

    Ok(())
}
