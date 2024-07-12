use std::collections::HashSet;

use crate::structs::Variable;
use crate::traits::to_uuid::ToUuid;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use crate::{utils::uuid::UuidHelpers, *};
use axum::extract::Path;
use sqlx::types::Uuid;
use uuid::Uuid as UuidValidator;

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
    user_id: UserId,
    Json(body): Json<NewVariableBody>,
) -> Result<Json<NewVariableReturnType>, AppError> {
    let project_id = body.project_id.to_uuid()?;

    if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let variable = sqlx::query!(
        "INSERT INTO variables (value, project_id, tag) VALUES ($1, $2, $3) RETURNING id",
        body.value,
        project_id,
        body.tag
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

#[derive(Serialize, Deserialize)]
pub struct UpdateManyBody {
    variables: Vec<Variable>,
}

pub async fn update_many_variables(
    State(state): State<AppState>,
    user_id: UserId,
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
        if !user_in_project(user_id.to_uuid(), project.id, &state.db).await? {
            return Err(AppError::Error(Errors::Unauthorized));
        }
    }

    // use UNNEST to update all the variables at once
    let variables = sqlx::query!(
        "UPDATE variables AS v SET value = u.value FROM UNNEST($1::uuid[], $2::text[]) AS u(id, value) WHERE v.id = u.id RETURNING v.id",
        &body.variables.iter().map(|v| v.id.to_uuid().unwrap()).collect::<Vec<Uuid>>(),
        &body.variables.iter().map(|v| v.value.clone()).collect::<Vec<String>>()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to update variables")?;

    dbg!(&variables);

    Ok(Json(
        variables
            .iter()
            .map(|v| v.id.to_string())
            .collect::<Vec<String>>(),
    ))
}

pub async fn delete_variable(
    State(state): State<AppState>,
    Path(id): Path<UuidValidator>,
    user_id: UserId,
) -> Result<(), AppError> {
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

    sqlx::query!("DELETE FROM variables WHERE id = $1", &id.to_sqlx())
        .execute(&*state.db)
        .await
        .context("Failed to delete variable")?;

    Ok(())
}

// #[derive(Serialize, Deserialize)]
// pub struct V2VariableInput {
//     value: String,
//     tag: Option<String>,
// }
//
// #[derive(Serialize, Deserialize)]
// pub struct V2SetManyBody {
//     project_id: String,
//     variables: Vec<V2VariableInput>,
// }
//
// #[derive(Serialize, Deserialize)]
// pub struct V2SetManyReturnType {
//     id: String,
// }
//
// pub async fn set_many_variables_v2(
//     State(state): State<AppState>,
//     user_id: UserId,
//     Json(body): Json<V2SetManyBody>,
// ) -> Result<Json<Vec<V2SetManyReturnType>>, AppError> {
//     let project_id = body.project_id.to_uuid()?;
//
//     if !user_in_project(user_id.to_uuid(), project_id, &state.db).await? {
//         return Err(AppError::Error(Errors::Unauthorized));
//     }
//
//     let variables = sqlx::query!(
//         "INSERT INTO variables (value, project_id, tag) SELECT * FROM UNNEST($1::text[], $2::uuid[], $3::text[]) RETURNING id",
//         &body.variables.iter().map(|v| v.value.clone()).collect::<Vec<String>>(),
//         &vec![project_id; body.variables.len()],
//         &body.variables.iter().map(|v| v.tag.clone()).collect::<Vec<Option<String>>>()
//     )
//     .fetch_all(&*state.db)
//     .await
//     .context("Failed to insert variables")?;
//
//     // let variables = sqlx::query!(
//     //     "INSERT INTO variables (value, project_id) SELECT * FROM UNNEST($1::text[], $2::uuid[]) RETURNING id",
//     //     &body.variables,
//     //     &vec![project_id; body.variables.len()]
//     // )
//     // .fetch_all(&*state.db)
//     // .await
//     // .context("Failed to insert variables")?;
//     //
//     // Ok(Json(
//     //     variables
//     //         .iter()
//     //         .map(|v| SetManyReturnType {
//     //             id: v.id.to_string(),
//     //         })
//     //         .collect::<Vec<SetManyReturnType>>(),
//     // ))
// }
