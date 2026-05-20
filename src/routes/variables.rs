use std::collections::HashSet;

use crate::extractors::client_ip::ClientIp;
use crate::helpers::caps;
use crate::helpers::caps::check_update_caps_grouped;
use crate::structs::Variable;
use crate::traits::to_uuid::ToUuid;
use crate::*;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use axum::extract::Path;
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
pub struct NewVariableBody {
    project_id: String,
    value: String,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NewVariableReturnType {
    id: String,
}

pub async fn new_variable(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<NewVariableBody>,
) -> Result<Json<NewVariableReturnType>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let values = [body.value.as_str()];
    caps::check_per_value(&state.caps, &values)?;
    caps::check_project_for_insert(&state.caps, &state.db, project_id, &values).await?;
    let delta = body.value.len() as i64;
    caps::check_user_total(&state.caps, &state.db, user_id, delta).await?;
    caps::check_and_record_ip(&state.caps, &state.db, ip, delta).await?;

    let variable = sqlx::query!(
        "INSERT INTO variables (value, project_id, tag) VALUES ($1, $2, $3) RETURNING id",
        body.value,
        project_id,
        body.tag.unwrap_or_default()
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
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<SetManyBody>,
) -> Result<Json<Vec<SetManyReturnType>>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let values: Vec<&str> = body.variables.iter().map(String::as_str).collect();
    caps::check_per_value(&state.caps, &values)?;
    caps::check_project_for_insert(&state.caps, &state.db, project_id, &values).await?;
    let delta: i64 = values.iter().map(|v| v.len() as i64).sum();
    caps::check_user_total(&state.caps, &state.db, user_id, delta).await?;
    caps::check_and_record_ip(&state.caps, &state.db, ip, delta).await?;

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

// literally what is the point of this method
pub async fn get_variable(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<String, AppError> {
    let variable = sqlx::query!(
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        variable_id
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get variable")?;

    if !user_in_project(user_id, variable.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(format!("variable: {}", variable.id))
}

#[derive(Serialize, Deserialize)]
pub struct UpdateManyBody {
    variables: Vec<Variable>,
}

pub async fn update_many_variables(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<UpdateManyBody>,
) -> Result<Json<Vec<String>>, AppError> {
    let projects = body
        .variables
        .iter()
        .map(|v| v.project_id.as_str())
        .collect::<HashSet<&str>>()
        .iter()
        .map(|s| s.to_string().to_uuid().unwrap())
        .collect::<Vec<Uuid>>();

    let projects = sqlx::query!(
        "SELECT id FROM projects WHERE id = ANY($1::uuid[])",
        &projects
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

    let all_values: Vec<&str> = body.variables.iter().map(|v| v.value.as_str()).collect();
    caps::check_per_value(&state.caps, &all_values)?;
    check_update_caps_grouped(
        &state,
        user_id,
        ip,
        body.variables
            .iter()
            .map(|v| (v.project_id.as_str(), v.id.as_str(), v.value.as_str()))
            .collect::<Vec<_>>(),
    )
    .await?;

    // use UNNEST to update all the variables at once
    let variables = sqlx::query!(
        "UPDATE variables AS v SET value = u.value FROM UNNEST($1::uuid[], $2::text[]) AS u(id, value) WHERE v.id = u.id RETURNING v.id",
        &body.variables.iter().map(|v| v.id.to_uuid().unwrap()).collect::<Vec<Uuid>>(),
        &body.variables.iter().map(|v| v.value.clone()).collect::<Vec<String>>()
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

pub async fn delete_variable(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<(), AppError> {
    let variable = sqlx::query!(
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        variable_id
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get variable")?;

    if !user_in_project(user_id, variable.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    sqlx::query!("DELETE FROM variables WHERE id = $1", variable_id)
        .execute(&*state.db)
        .await
        .context("Failed to delete variable")?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct V2VariableInput {
    value: String,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct V2SetManyBody {
    project_id: String,
    variables: Vec<V2VariableInput>,
}

#[derive(Serialize, Deserialize)]
pub struct V2SetManyReturnType {
    id: String,
}

pub async fn set_many_variables_v2(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<V2SetManyBody>,
) -> Result<Json<Vec<V2SetManyReturnType>>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let values: Vec<&str> = body.variables.iter().map(|v| v.value.as_str()).collect();
    caps::check_per_value(&state.caps, &values)?;
    caps::check_project_for_insert(&state.caps, &state.db, project_id, &values).await?;
    let delta: i64 = values.iter().map(|v| v.len() as i64).sum();
    caps::check_user_total(&state.caps, &state.db, user_id, delta).await?;
    caps::check_and_record_ip(&state.caps, &state.db, ip, delta).await?;

    let variables = sqlx::query!(
        "INSERT INTO variables (value, project_id, tag) SELECT * FROM UNNEST($1::text[], $2::uuid[], $3::text[]) RETURNING id",
        &body.variables.iter().map(|v| v.value.clone()).collect::<Vec<String>>(),
        &vec![project_id; body.variables.len()],
        // &body.variables.iter().map(|v| v.tag.clone()).collect::<Vec<Option<String>>>()
        &body.variables.iter().map(|v| v.tag.clone().unwrap_or_default()).collect::<Vec<String>>()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to insert variables")?;

    Ok(Json(
        variables
            .iter()
            .map(|v| V2SetManyReturnType {
                id: v.id.to_string(),
            })
            .collect::<Vec<V2SetManyReturnType>>(),
    ))

    // let variables = sqlx::query!(
    //     "INSERT INTO variables (value, project_id) SELECT * FROM UNNEST($1::text[], $2::uuid[]) RETURNING id",
    //     &body.variables,
    //     &vec![project_id; body.variables.len()]
    // )
    // .fetch_all(&*state.db)
    // .await
    // .context("Failed to insert variables")?;
    //
    // Ok(Json(
    //     variables
    //         .iter()
    //         .map(|v| SetManyReturnType {
    //             id: v.id.to_string(),
    //         })
    //         .collect::<Vec<SetManyReturnType>>(),
    // ))
}
