use super::{traits::to_uuid::ToUuid, *};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AddUserBody {
    user_id: String,
}

#[utoipa::path(
    post,
    path = "/{project_id}",
    tag = PROJECT_TAG,
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
    Json(body): Json<AddUserBody>,
) -> Result<(), AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let user_to_insert = body.user_id.to_uuid()?;

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id) VALUES ($1, $2)",
        user_to_insert,
        project_id
    )
    .execute(&*state.db)
    .await
    .context("Failed to add user to project")?;

    Ok(())
}
