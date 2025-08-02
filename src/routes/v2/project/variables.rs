use super::{structs::PartialKey, *};

#[utoipa::path(
    get,
    path = "/{project_id}/variables",
    tag = PROJECT_TAG,
    responses(
        (status = 200,
            description = "Success. Returns a list of variables for the project",    
            body = Vec<PartialKey>),
    (status = 401, description = "Unauthorized. User not in project"),
        // (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn variables(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<PartialKey>>, AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let variables = sqlx::query_as!(
        PartialKey,
        "SELECT id, value, project_id, created_at 
            FROM variables 
            WHERE project_id = $1",
        project_id
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get variables")?;

    Ok(Json(variables))
}
