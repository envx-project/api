use super::*;

#[utoipa::path(
    post,
    path = "/set-many",
    tag = VARIABLES_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    )
)]
pub async fn set_many() {}
