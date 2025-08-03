use super::*;

#[utoipa::path(
    put,
    path = "/update-many",
    tag = VARIABLES_TAG,
    responses(
        // (status = 200, description = "Project ID", body = String),
        // (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn update_many() {
    todo!();
}
