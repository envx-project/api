use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::{extractors::user::UserId, structs::ProjectInvite};

use super::*;

#[derive(Deserialize, ToSchema)]
pub struct AcceptInviteBody {
    pub code: Uuid,
    pub verifier: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct AcceptInviteReturnType {
    pub id: String,
    pub ciphertext: String,
}

#[utoipa::path(
    post,
    path = "/accept",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Success", body = AcceptInviteReturnType),
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
    UserId(user_id): UserId,
    Json(body): Json<AcceptInviteBody>,
) -> Result<Json<AcceptInviteReturnType>, AppError> {
    let invite = sqlx::query_as!(
        ProjectInvite,
        "SELECT * FROM project_invites WHERE id = $1",
        body.code
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

    if invite.invited_id.is_some() {
        return Err(AppError::Generic(
            StatusCode::CONFLICT,
            "Invite already accepted".into(),
        ));
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
        body.code
    )
    .fetch_optional(&*state.db)
    .await
    .context("Failed to update invite")?;

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING",
        &user_id,
        &invite.project_id,
    )
    .execute(&*state.db)
    .await
    .context("Failed to insert user project relations")?;

    if id.is_none() {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(Json(AcceptInviteReturnType {
        ciphertext: invite.ciphertext.unwrap(),
        id: id.unwrap().id.to_string(),
    }))
}
