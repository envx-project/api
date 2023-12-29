use crate::structs::{PartialKey, User};
use crate::traits::to_uuid::ToUuid;
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

pub async fn get_project_variables(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<UuidValidator>,
) -> Result<Json<Vec<PartialKey>>, AppError> {
    let project_id = id.to_sqlx();

    if !user_in_project(user_id.to_uuid(), id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let variables = sqlx::query!(
        "SELECT id, value, project_id, created_at FROM variables WHERE project_id = $1",
        project_id
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get variables")?;

    let partial_keys = variables
        .iter()
        .map(|variable| PartialKey {
            id: variable.id.to_string(),
            value: variable.value.clone(),
            project_id: variable.project_id.to_string(),
            created_at: variable.created_at.to_string(),
        })
        .collect::<Vec<PartialKey>>();

    Ok(Json(partial_keys))
}

#[derive(Serialize, Deserialize)]
pub struct AddUserBody {
    user_id: String,
}
pub async fn add_user(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<UuidValidator>,
    Json(body): Json<AddUserBody>,
) -> Result<(), AppError> {
    let project_id = id.to_sqlx();
    if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
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

pub async fn list_projects(
    State(state): State<AppState>,
    user_id: UserId,
) -> Result<Json<Vec<String>>, AppError> {
    let projects = sqlx::query!(
        "SELECT p.id FROM projects p
        JOIN user_project_relations upr ON p.id = upr.project_id
        WHERE upr.user_id = $1",
        user_id.to_uuid()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get projects")?;

    let projects = projects
        .iter()
        .map(|project| project.id.to_string())
        .collect::<Vec<String>>();

    Ok(Json(projects))
}
