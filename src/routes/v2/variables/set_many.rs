use crate::extractors::client_ip::ClientIp;

use super::*;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct VariableInput {
    value: String,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SetManyBody {
    project_id: Uuid,
    variables: Vec<VariableInput>,
}

// TODO: I'm pretty sure this should be on projects/set-many instead but whatever
#[utoipa::path(
    post,
    path = "/set-many",
    tag = VARIABLES_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn set_many(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<SetManyBody>,
) -> Result<Json<Vec<Uuid>>, AppError> {
    let (values, tags) = body
        .variables
        .into_iter()
        .map(|v| (v.value, v.tag.unwrap_or_default()))
        .unzip();
    Ok(Json(
        crate::helpers::variables::insert_many(&state, user_id, ip, body.project_id, values, tags)
            .await?,
    ))
}
