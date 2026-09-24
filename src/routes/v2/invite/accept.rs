use super::*;
use crate::{
    extractors::{client_ip::ClientIp, user::UserId},
    helpers::project_snapshot::{self as snapshots, Recipient, RewrappedVariable},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use uuid::Uuid;

#[derive(Clone, Deserialize, ToSchema)]
pub struct PrepareInviteBody {
    pub code: Uuid,
    pub verifier: Uuid,
}
#[derive(Clone, Deserialize, ToSchema)]
pub struct AcceptInviteBody {
    pub code: Uuid,
    pub verifier: Uuid,
    pub protocol_version: Option<u8>,
    pub snapshot: Option<String>,
    pub variables: Option<Vec<RewrappedVariable>>,
}
#[derive(Serialize, ToSchema)]
pub struct PreparedInvite {
    pub protocol_version: u8,
    pub project_id: Uuid,
    pub invite_id: Uuid,
    pub ciphertext: String,
    pub source_snapshot: String,
    pub snapshot: String,
    pub users: Vec<Recipient>,
}
#[derive(Serialize, ToSchema)]
pub struct AcceptInviteReturnType {
    pub project_id: Uuid,
    pub invite_id: Uuid,
}
#[derive(sqlx::FromRow)]
struct Invite {
    project_id: Uuid,
    author_id: Uuid,
    verifier_argon2id: String,
    ciphertext: Option<String>,
    snapshot_hash: Option<String>,
    invited_id: Option<Uuid>,
    expires_at: chrono::DateTime<chrono::Utc>,
}
// The initial lookup only chooses the project lock. All authorization and content
// checks use a fresh locked invitation after that lock has been acquired.
async fn lock_invite(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: Uuid,
    verifier: Uuid,
) -> Result<Invite, AppError> {
    let project: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM project_invites WHERE id=$1")
            .bind(code)
            .fetch_optional(&mut **tx)
            .await?;
    snapshots::lock_project(tx, project.ok_or(Errors::NotFound)?).await?;
    let invite: Invite = sqlx::query_as("SELECT project_id,author_id,verifier_argon2id,ciphertext,snapshot_hash,invited_id,expires_at FROM project_invites WHERE id=$1 FOR UPDATE")
        .bind(code).fetch_one(&mut **tx).await?;
    let hash = PasswordHash::new(&invite.verifier_argon2id)
        .map_err(|_| AppError::Error(Errors::Unauthorized))?;
    Argon2::default()
        .verify_password(verifier.as_bytes(), &hash)
        .map_err(|_| AppError::Error(Errors::Unauthorized))?;
    if invite.invited_id.is_some() {
        return Err(snapshots::conflict(
            "invite_already_accepted",
            "Invitation already accepted",
        ));
    }
    if invite.expires_at <= chrono::Utc::now() || invite.ciphertext.is_none() {
        return Err(Errors::Unauthorized.into());
    }
    snapshots::authorize(tx, invite.project_id, invite.author_id).await?;
    let expected = invite
        .snapshot_hash
        .as_deref()
        .ok_or_else(snapshots::upgrade)?;
    let current = snapshots::read(tx, invite.project_id, &[]).await?;
    if current.snapshot != expected {
        return Err(snapshots::stale());
    }
    Ok(invite)
}
#[utoipa::path(post,path="/prepare",tag=INVITE_TAG,responses((status=200,body=PreparedInvite),(status=409,description="Upgrade or regenerate invitation")),security(("bearer"=[])))]
pub async fn prepare_invite(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<PrepareInviteBody>,
) -> Result<Json<PreparedInvite>, AppError> {
    let mut tx = state.db.begin().await?;
    let invite = lock_invite(&mut tx, body.code, body.verifier).await?;
    let current = snapshots::read(&mut tx, invite.project_id, &[user]).await?;
    tx.commit().await?;
    Ok(Json(PreparedInvite {
        protocol_version: snapshots::VERSION,
        project_id: invite.project_id,
        invite_id: body.code,
        ciphertext: invite.ciphertext.unwrap(),
        source_snapshot: invite.snapshot_hash.unwrap(),
        snapshot: current.snapshot,
        users: current.users,
    }))
}
#[utoipa::path(post,path="/accept",tag=INVITE_TAG,responses((status=200,body=AcceptInviteReturnType),(status=409,description="Upgrade or regenerate invitation")),security(("bearer"=[])))]
pub async fn accept_invite(
    State(state): State<AppState>,
    UserId(user): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<AcceptInviteBody>,
) -> Result<Json<AcceptInviteReturnType>, AppError> {
    snapshots::require_version(body.protocol_version)?;
    let expected = body.snapshot.as_deref().ok_or_else(snapshots::upgrade)?;
    let variables = body.variables.as_deref().ok_or_else(snapshots::upgrade)?;
    let mut tx = state.db.begin().await?;
    let invite = lock_invite(&mut tx, body.code, body.verifier).await?;
    let current = snapshots::read(&mut tx, invite.project_id, &[user]).await?;
    snapshots::rewrap(&state, &mut tx, &current, expected, variables, user, ip).await?;
    sqlx::query("UPDATE project_invites SET invited_id=$1,ciphertext=NULL WHERE id=$2")
        .bind(user)
        .bind(body.code)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO user_project_relations(user_id,project_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
        .bind(user).bind(invite.project_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(AcceptInviteReturnType {
        project_id: invite.project_id,
        invite_id: body.code,
    }))
}

#[cfg(test)]
mod tests;
