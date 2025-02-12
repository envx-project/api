/// Get health of the API.
#[utoipa::path(
    get,
    path = "/.well-known/health-check",
    responses(
        (status = OK, description = "Success", body = str, content_type = "text/plain")
    )
)]
pub async fn health_check() -> &'static str {
    "ok"
}
