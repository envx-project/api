use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{extractors::user::UserId, helpers::project::user_in_project, *};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct InviteBody {
    project_id: Uuid,
    verifier: String,
    exp: DateTime<Utc>,
    ciphertext: String,
}

#[utoipa::path(
    post,
    path = "/new",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn new_invite(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(body): Json<InviteBody>,
) -> Result<String, AppError> {
    if !user_in_project(user_id, body.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let verifier_hash = Argon2::default()
        .hash_password(body.verifier.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|e| AppError::Generic(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .to_string();

    let invite_code = Uuid::new_v4();

    sqlx::query!(
        "INSERT INTO project_invites (
            project_id,
            author_id,
            expires_at,
            verifier_argon2id,
            ciphertext,
            id
        )
        VALUES ($1, $2, $3, $4, $5, $6)",
        body.project_id,
        user_id,
        body.exp,
        verifier_hash,
        body.ciphertext,
        invite_code
    )
    .execute(&*state.db)
    .await
    .context("Failed to insert project invite")?;

    Ok(invite_code.into())
}
