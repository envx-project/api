use super::*;
use uuid::Uuid;

#[utoipa::path(
    delete,
    path = "/{variable_id}",
    tag = VARIABLES_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<(), AppError> {
    crate::helpers::variables::delete(&state, user_id, variable_id).await
}
