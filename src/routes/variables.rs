use crate::{extractors::user::UserId, helpers::project::user_in_project, traits::to_uuid::ToUuid};
use crate::{utils::uuid::UuidHelpers, *};
use axum::extract::Path;
use uuid::Uuid as UuidValidator;

#[derive(Serialize, Deserialize)]
pub struct NewVariableBody {
    project_id: String,
    value: String,
}

#[derive(Serialize, Deserialize)]
pub struct NewVariableReturnType {
    id: String,
}

pub async fn new_variable(
    State(state): State<AppState>,
    user_id: UserId,
    Json(body): Json<NewVariableBody>,
) -> Result<Json<NewVariableReturnType>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let variable = sqlx::query!(
        "INSERT INTO variables (value, project_id) VALUES ($1, $2) RETURNING id",
        body.value,
        project_id
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to insert variable")?;

    Ok(Json(NewVariableReturnType {
        id: variable.id.to_string(),
    }))
}

#[derive(Serialize, Deserialize)]
pub struct SetManyBody {
    project_id: String,
    variables: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SetManyReturnType {
    id: String,
}

pub async fn set_many_variables(
    State(state): State<AppState>,
    user_id: UserId,
    Json(body): Json<SetManyBody>,
) -> Result<Json<Vec<SetManyReturnType>>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let variables = sqlx::query!(
        "INSERT INTO variables (value, project_id) SELECT * FROM UNNEST($1::text[], $2::uuid[]) RETURNING id",
        &body.variables,
        &vec![project_id; body.variables.len()]
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to insert variables")?;

    Ok(Json(
        variables
            .iter()
            .map(|v| SetManyReturnType {
                id: v.id.to_string(),
            })
            .collect::<Vec<SetManyReturnType>>(),
    ))
}

pub async fn get_variable(
    State(state): State<AppState>,
    Path(id): Path<UuidValidator>,
    user_id: UserId,
) -> Result<String, AppError> {
    let variable = sqlx::query!(
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        &id.to_sqlx()
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get variable")?;

    if !user_in_project(user_id.to_uuid(), variable.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(format!("variable: {}", variable.id))
}
