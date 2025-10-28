use crate::structs::{PartialKey, User};
use crate::traits::to_uuid::ToUuid;
use crate::*;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use axum::extract::Path;
use uuid::Uuid as UuidValidator;

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    project_id: uuid::Uuid,
    users: Vec<User>,
}

pub async fn get_project_info(
    UserId(user_id): UserId,
    State(state): State<AppState>,
    Path(project_id): Path<uuid::Uuid>,
) -> Result<Json<ProjectInfo>, AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
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

    Ok(Json(ProjectInfo { project_id, users }))
}

pub async fn get_project_variables(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<PartialKey>>, AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
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
            created_at: variable.created_at,
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
    UserId(user_id): UserId,
    Path(project_id): Path<uuid::Uuid>,
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

#[derive(Serialize, Deserialize)]
pub struct RemoveUserBody {
    users: Vec<String>,
}

pub async fn remove_user(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<uuid::Uuid>,
    Json(body): Json<RemoveUserBody>,
) -> Result<(), AppError> {
    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let users_to_remove = body
        .users
        .iter()
        .map(|user| user.to_uuid().unwrap())
        .collect::<Vec<UuidValidator>>();

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

pub async fn list_projects(
    State(state): State<AppState>,
    UserId(user_id): UserId,
) -> Result<Json<Vec<String>>, AppError> {
    let projects = sqlx::query!(
        "SELECT p.id FROM projects p
        JOIN user_project_relations upr ON p.id = upr.project_id
        WHERE upr.user_id = $1",
        user_id
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

pub async fn new_project(
    State(state): State<AppState>,
    UserId(user_id): UserId,
) -> Result<String, AppError> {
    let project = sqlx::query!("INSERT INTO projects DEFAULT VALUES RETURNING id",)
        .fetch_one(&*state.db)
        .await
        .context("Failed to create project")?;

    let project_id = project.id;

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id) VALUES ($1, $2)",
        user_id,
        project_id
    )
    .execute(&*state.db)
    .await
    .context("Failed to add user to project")?;

    Ok(project_id.to_string())
}
