use std::collections::HashSet;

use crate::{structs::Variable, traits::to_uuid::ToUuid};

use super::*;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateManyBody {
    variables: Vec<Variable>,
}

#[utoipa::path(
    put,
    path = "/update-many",
    tag = VARIABLES_TAG,
    responses(
        (status = 200, description = "Project ID", body = Vec<String>),
        (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn update_many(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(body): Json<UpdateManyBody>,
) -> Result<Json<Vec<String>>, AppError> {
    let project_ids = body
        .variables
        .iter()
        .map(|v| v.project_id.as_str())
        .collect::<HashSet<&str>>()
        .iter()
        .map(|s| s.to_string().to_uuid())
        .collect::<Result<Vec<Uuid>, _>>()?;

    let projects = sqlx::query!(
        "SELECT id FROM projects WHERE id = ANY($1::uuid[])",
        &project_ids
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get projects")?;

    // make sure the user is in all the projects
    for project in projects {
        if !user_in_project(user_id, project.id, &state.db).await? {
            return Err(AppError::Error(Errors::Unauthorized));
        }
    }

    // use UNNEST to update all the variables at once
    let variables = sqlx::query!(
        "UPDATE variables AS v 
        SET value = u.value 
            FROM UNNEST($1::uuid[], $2::text[]) AS u(id, value) 
        WHERE v.id = u.id 
        RETURNING v.id",
        &body
            .variables
            .iter()
            .map(|v| v.id.to_uuid().unwrap())
            .collect::<Vec<Uuid>>(),
        &body
            .variables
            .iter()
            .map(|v| v.value.clone())
            .collect::<Vec<String>>()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to update variables")?;

    Ok(Json(
        variables
            .iter()
            .map(|v| v.id.to_string())
            .collect::<Vec<String>>(),
    ))
}
