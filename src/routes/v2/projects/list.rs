use super::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct ListProjectsV2 {
    project_id: String,
    project_name: String,
}

pub async fn list_projects_v2(
    State(state): State<AppState>,
    user_id: UserId,
) -> Result<Json<Vec<ListProjectsV2>>, AppError> {
    let projects = sqlx::query!(
        "SELECT p.id, p.name FROM projects p
        JOIN user_project_relations upr ON p.id = upr.project_id
        WHERE upr.user_id = $1",
        user_id.to_uuid()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get projects")?;

    let projects = projects
        .iter()
        .map(|project| ListProjectsV2 {
            project_id: project.id.to_string(),
            project_name: project.name.clone(),
        })
        .collect::<Vec<ListProjectsV2>>();

    Ok(Json(projects))
}
