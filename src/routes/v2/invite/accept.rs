use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::Path;
use uuid::Uuid;

use crate::{extractors::user::UserId, structs::ProjectInvite};

use super::*;

#[derive(Deserialize, ToSchema)]
pub struct AcceptInviteBody {
    pub verifier: String,
}

#[utoipa::path(
    post,
    path = "/accept/{invite_code}",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
        (status = 404, description = "Invite not found"),
        (status = 409, description = "Invite already accepted"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn accept_invite(
    State(state): State<AppState>,
    Path(invite_code): Path<Uuid>,
    UserId(user_id): UserId,
    Json(body): Json<AcceptInviteBody>,
) -> Result<String, AppError> {
    let invite = sqlx::query_as!(
        ProjectInvite,
        "SELECT * FROM project_invites WHERE id = $1",
        invite_code
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to fetch invite")?;

    let parsed_hash = PasswordHash::new(&invite.verifier_argon2id)
        .map_err(|_| AppError::Error(Errors::Unauthorized))?;
    let is_valid = Argon2::default()
        .verify_password(body.verifier.as_bytes(), &parsed_hash)
        .is_ok();

    if !is_valid {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    let id = sqlx::query!(
        "UPDATE project_invites 
        SET invited_id = $1,
        ciphertext = NULL 
        WHERE id = $2
            AND invited_id IS NULL
            AND ciphertext IS NOT NULL
            AND expires_at > NOW()
        RETURNING id;
        ",
        user_id,
        invite_code
    )
    .fetch_optional(&*state.db)
    .await
    .context("Failed to update invite")?;

    if id.is_none() {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(invite.ciphertext.unwrap())
}
