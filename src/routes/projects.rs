use crate::structs::User;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use crate::{utils::uuid::UuidHelpers, *};
use axum::extract::Path;
use uuid::Uuid as UuidValidator;

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    project_id: String,
    users: Vec<User>,
}

pub async fn get_project_info(
    user_id: UserId,
    State(state): State<AppState>,
    Path(id): Path<UuidValidator>,
) -> Result<Json<ProjectInfo>, AppError> {
    let project_id = id.to_sqlx();

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

    Ok(Json(ProjectInfo {
        project_id: id.to_string(),
        users,
    }))
}
