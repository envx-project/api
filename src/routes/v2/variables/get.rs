use super::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Variable {
    id: Uuid,
    value: String,
    project_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{variable_id}",
    tag = VARIABLES_TAG,
    responses(
        (status = 200, description = "Success", body = Variable),
        (status = 404, description = "Variable not found"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<Json<Variable>, AppError> {
    let variable = sqlx::query_as!(
        Variable,
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        variable_id
    )
    .fetch_one(&*state.db)
    .await;

    let variable = match variable {
        Ok(variable) => variable,
        Err(_) => return Err(AppError::Error(Errors::NotFound)),
    };

    if !user_in_project(user_id, variable.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(Json(variable))
}
