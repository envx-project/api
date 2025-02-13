use super::*;

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct UpdateProjectV2 {
    project_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct UpdateProjectV2Response {
    id: String,
}

#[utoipa::path(
    put,
    path = "/{id}",
    responses(
        (status = OK, description = "Success", body = UpdateProjectV2Response, content_type = "application/json"),
        // (status = UNAUTHORIZED, description = "Unauthorized", body = Errors, content_type = "application/json")
    ),
    tag = super::PROJECT_TAG
)]
pub async fn update_project_v2(
    user_id: UserId,
    State(state): State<AppState>,
    Path(id): Path<UuidValidator>,
    Json(update_project): Json<UpdateProjectV2>,
) -> Result<Json<UpdateProjectV2Response>, AppError> {
    let project_id = id.to_sqlx();

    if !user_in_project(user_id.to_uuid(), id, &state.db).await? {
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

    Ok(Json(UpdateProjectV2Response { id: id.to_string() }))
}
