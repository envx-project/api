use crate::db;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
pub struct GetProjectInfoReturnType {
    project_id: String,
    users: Vec<String>,
}
pub async fn get_project_info(project_id: String) -> anyhow::Result<GetProjectInfoReturnType> {
    let db = db::db().await?;

    let uuid_id = Uuid::parse_str(&project_id)?;

    let users = sqlx::query!(
        "SELECT user_id FROM user_project_relations WHERE project_id = $1",
        uuid_id
    )
    .fetch_all(&db)
    .await?;

    Ok(GetProjectInfoReturnType {
        project_id: project_id.clone(),
        users: users.iter().map(|user| user.user_id.to_string()).collect(),
    })
}

pub async fn user_in_project(user_id: String, project_id: String) -> anyhow::Result<bool> {
    let db = db::db().await?;

    let uuid_user_id = Uuid::parse_str(&user_id)?;
    let uuid_project_id = Uuid::parse_str(&project_id)?;

    let user = sqlx::query!(
        "SELECT user_id FROM user_project_relations WHERE user_id = $1 AND project_id = $2",
        uuid_user_id,
        uuid_project_id
    )
    .fetch_optional(&db)
    .await?;

    Ok(user.is_some())
}
