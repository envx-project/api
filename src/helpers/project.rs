use crate::state::DB;
use sqlx::types::Uuid;

pub async fn user_in_project(user_id: Uuid, project_id: Uuid, db: &DB) -> anyhow::Result<bool> {
    let user = sqlx::query!(
        "SELECT user_id FROM user_project_relations WHERE user_id = $1 AND project_id = $2",
        user_id,
        project_id
    )
    .fetch_optional(db.as_ref())
    .await?;

    Ok(user.is_some())
}
