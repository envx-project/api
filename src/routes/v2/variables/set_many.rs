use crate::extractors::client_ip::ClientIp;
use crate::helpers::caps;

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
    let project_id = body.project_id;

    if !user_in_project(user_id, project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let (values, tags): (Vec<_>, Vec<_>) = body
        .variables
        .into_iter()
        .map(|v| (v.value, v.tag.unwrap_or_default()))
        .unzip();

    let value_refs: Vec<&str> = values.iter().map(String::as_str).collect();
    caps::check_per_value(&state.caps, &value_refs)?;
    caps::check_project_for_insert(&state.caps, &state.db, project_id, &value_refs).await?;
    let delta: i64 = value_refs.iter().map(|v| v.len() as i64).sum();
    caps::check_user_total(&state.caps, &state.db, user_id, delta).await?;
    caps::check_and_record_ip(&state.caps, &state.db, ip, delta).await?;

    let variables = sqlx::query!(
        "INSERT INTO variables (value, project_id, tag)
        SELECT value, $2::uuid, tag 
        FROM UNNEST($1::text[], $3::text[]) AS t(value, tag)
        RETURNING id",
        &values,
        project_id,
        &tags,
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to insert variables")?;

    Ok(Json(variables.iter().map(|v| v.id).collect::<Vec<_>>()))
}
