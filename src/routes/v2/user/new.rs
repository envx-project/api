use super::*;
use pgp::{Deserializable, SignedPublicKey};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct NewUserBody {
    username: String,
    public_key: String,
}

#[utoipa::path(
    post,
    path = "/new",
    tag = USER_TAG,
    request_body = NewUserBody,
    responses(
        (status = 200, description = "User ID", body = String),
        (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn new_user_v2(
    State(state): State<AppState>,
    Json(body): Json<NewUserBody>,
) -> Result<String, AppError> {
    // public key validation
    match SignedPublicKey::from_string(&body.public_key) {
        Ok(_) => {}
        Err(_) => {
            return Err(AppError::Error(Errors::InvalidPublicKey));
        }
    }

    let user = sqlx::query!(
        "INSERT INTO users (username, public_key) VALUES ($1, $2) RETURNING id",
        body.username,
        body.public_key
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to insert user")?;

    Ok(user.id.to_string())
}
