use super::{traits::to_uuid::ToUuid, *};
use uuid::Uuid;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct RemoveUserBody {
    user_ids: Vec<String>,
}

#[utoipa::path(
    delete,
    path = "/{project_id}",
    tag = PROJECT_TAG,
    security(
        ("bearer" = []),
    ),
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn remove_users(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RemoveUserBody>,
) -> Result<(), AppError> {
    let users = body
        .user_ids
        .iter()
        .map(|user| user.to_uuid())
        .collect::<Result<Vec<Uuid>, _>>()?;
    crate::helpers::project_snapshot::remove_members(&state, project_id, user_id, &users).await
}
