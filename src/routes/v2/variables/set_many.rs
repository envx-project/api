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
    )
)]
pub async fn set_many(
    State(state): State<AppState>,
    UserId(user_id): UserId,
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
