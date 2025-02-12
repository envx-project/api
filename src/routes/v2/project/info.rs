use super::*;

#[derive(Serialize, Deserialize)]
pub struct ProjectInfoV2 {
    project_id: String,
    project_name: String,
    users: Vec<User>,
}

pub async fn get_project_info_v2(
    user_id: UserId,
    State(state): State<AppState>,
    Path(id): Path<UuidValidator>,
) -> Result<Json<ProjectInfoV2>, AppError> {
    let project_id = id.to_sqlx();

    let project_name = sqlx::query!("SELECT name FROM projects WHERE id = $1", project_id)
        .fetch_one(&*state.db)
        .await
        .context("Failed to get project name")?;

    if !user_in_project(user_id.to_uuid(), id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let users = sqlx::query!(
        "SELECT u.id, u.username, u.created_at, u.public_key
        FROM users u
        JOIN user_project_relations upr ON u.id = upr.user_id
        WHERE upr.project_id = $1",
        project_id
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get users")?;

    let users: Vec<User> = users
        .iter()
        .map(|user| User {
            id: user.id.to_string(),
            username: user.username.clone(),
            created_at: user.created_at.to_string(),
            public_key: user.public_key.clone(),
        })
        .collect();

    Ok(Json(ProjectInfoV2 {
        project_id: id.to_string(),
        project_name: project_name.name,
        users,
    }))
}
