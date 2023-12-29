use crate::{extractors::user::UserId, helpers::project::user_in_project, traits::to_uuid::ToUuid};
use axum::extract::Path;
use uuid::Uuid as UuidValidator;

use crate::{utils::uuid::UuidHelpers, *};

#[derive(Serialize, Deserialize)]
pub struct Body {
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
    Json(body): Json<Body>,
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

pub async fn set_many_variables(
    State(state): State<AppState>,
    user_id: UserId,
    Json(body): Json<Vec<Body>>,
) -> Result<Json<NewVariableReturnType>, AppError> {
    let project_id = body[0].project_id.to_uuid()?;

    if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let mut query = String::from("INSERT INTO variables (value, project_id) VALUES ");

    let mut values = Vec::new();

    for (i, variable) in body.iter().enumerate() {
        query.push_str(&format!("(${}, ${})", i * 2 + 1, i * 2 + 2));

        if i != body.len() - 1 {
            query.push_str(", ");
        }

        values.push(variable.value.clone());
        values.push(project_id);
    }

    query.push_str(" RETURNING id");

    let variable = sqlx::query(&query)
        .bind_all(values)
        .fetch_one(&*state.db)
        .await
        .context("Failed to insert variable")?;

    Ok(Json(NewVariableReturnType {
        id: variable.id.to_string(),
    }))
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

    Ok(String::from(format!("variable: {}", variable.id)))
}
