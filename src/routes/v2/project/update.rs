use super::*;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct UpdateProjectV2 {
    project_name: Option<String>,
}

#[utoipa::path(
    put,
    path = "/{project_id}",
    responses(
        (status = OK, description = "Success"),
        // (status = UNAUTHORIZED, description = "Unauthorized", body = Errors, content_type = "application/json")
    ),
    tag = super::PROJECT_TAG
)]
pub async fn update_project_v2(
    UserId(user_id): UserId,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(update_project): Json<UpdateProjectV2>,
) -> Result<(), AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    sqlx::query!(
        "UPDATE projects SET name = COALESCE($1, name) WHERE id = $2",
        update_project.project_name,
        project_id
    )
    .execute(&*state.db)
    .await
    .context("Failed to update project")?;

    Ok(())
}
