use super::*;

#[utoipa::path(
    get,
    path = "/{id}",
    tag = VARIABLES_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    )
)]
pub async fn get() {}
