use super::*;

#[utoipa::path(
    post,
    path = "/new",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Project ID", body = String),
        (status = 400, description = "Invalid public key"),
    )
)]
pub async fn new_invite() {}
