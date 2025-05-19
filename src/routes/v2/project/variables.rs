use super::*;

#[utoipa::path(
    get,
    path = "/{project_id}/variables",
    tag = PROJECT_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    )
)]
pub async fn variables() {}
