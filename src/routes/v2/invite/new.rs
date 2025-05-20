use chrono::{DateTime, Utc};
use pgp::{Deserializable, Message};
use uuid::Uuid;

use super::{
    extractors::user::UserId, helpers::project::user_in_project, utils::rpgp::verify_signature, *,
};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct InviteBody {
    invite_code: Uuid,
    project_id: Uuid,
    author_signature: String,
    exp: DateTime<Utc>,
}

#[utoipa::path(
    post,
    path = "/new",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Invalid public key"),
    )
)]
pub async fn new_invite(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(body): Json<InviteBody>,
) -> Result<(), AppError> {
    unimplemented!();

    if !user_in_project(user_id, body.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    {
        let user_pubkey = sqlx::query!("SELECT public_key FROM users WHERE id = $1", user_id)
            .fetch_one(&*state.db)
            .await?
            .public_key;
        let (author_signature, _) = Message::from_string(&body.author_signature)
            .context("Failed to parse author signature")?;
        if !verify_signature(author_signature, user_pubkey)? {
            return Err(AppError::Error(Errors::Unauthorized));
        }
    }

    sqlx::query!(
        "INSERT INTO project_invites (
            project_id,
            author_id,
            author_signature,
            expires_at
        )
        VALUES ($1, $2, $3, $4)",
        body.project_id,
        user_id,
        body.author_signature,
        body.exp
    )
    .execute(&*state.db)
    .await
    .context("Failed to insert project invite")?;

    Ok(())
}
