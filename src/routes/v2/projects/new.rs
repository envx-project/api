use super::*;

#[derive(Serialize, Deserialize)]
pub struct NewProjectBody {
    name: String,
}

pub async fn new_project_v2(
    State(state): State<AppState>,
    user_id: UserId,
    Json(body): Json<NewProjectBody>,
) -> Result<String, AppError> {
    let mut tx = state
        .db
        .begin()
        .await
        .context("Failed to begin transaction")?;

    let project = sqlx::query!(
        "INSERT INTO projects (name) VALUES ($1) RETURNING id",
        body.name
    )
    .fetch_one(&mut *tx)
    .await
    .context("Failed to create project")?;

    let project_id = project.id;

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id) VALUES ($1, $2)",
        user_id.to_uuid(),
        project_id
    )
    .execute(&mut *tx)
    .await
    .context("Failed to add user to project")?;

    tx.commit().await.context("Failed to commit transaction")?;

    Ok(project_id.to_string())
}
