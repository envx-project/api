use super::*;
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/{project_id}",
    tag = PROJECT_TAG,
    security(
        ("bearer" = []),
    ),
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
        (status = 401, description = "Unauthorized. User not in project"),
    )
)]
pub async fn add_user(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<sqlx::types::Uuid>,
    Json(users_to_add): Json<Vec<Uuid>>,
) -> Result<(), AppError> {
    let _ = (state, user_id, project_id, users_to_add);
    Err(crate::helpers::project_snapshot::upgrade())
}
