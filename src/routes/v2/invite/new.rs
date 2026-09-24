use super::*;
use crate::{extractors::user::UserId, helpers::project_snapshot as snapshots};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::http::StatusCode;
use uuid::Uuid;

const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_ACTIVE_INVITES: i64 = 32;
const MAX_ACTIVE_BYTES: i64 = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct InviteBody {
    pub project_id: Uuid,
    pub ciphertext: String,
    pub protocol_version: Option<u8>,
    pub snapshot: Option<String>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct InviteResponse {
    pub invite_code: Uuid,
    pub verifier: Uuid,
}
#[utoipa::path(post,path="/new",tag=INVITE_TAG,responses((status=200,body=InviteResponse),(status=409,description="Upgrade or regenerate invitation")),security(("bearer"=[])))]
pub async fn new_invite(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<InviteBody>,
) -> Result<Json<InviteResponse>, AppError> {
    snapshots::require_version(body.protocol_version)?;
    if body.ciphertext.is_empty() || body.ciphertext.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::Generic(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Invitation ciphertext must be between 1 byte and 1 MiB".into(),
        ));
    }
    let expected = body.snapshot.as_deref().ok_or_else(snapshots::upgrade)?;
    let verifier = Uuid::new_v4();
    let hash = Argon2::default()
        .hash_password(verifier.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|e| AppError::Generic(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .to_string();
    let mut tx = state.db.begin().await?;
    snapshots::lock_project(&mut tx, body.project_id).await?;
    snapshots::authorize(&mut tx, body.project_id, user).await?;
    let current = snapshots::read(&mut tx, body.project_id, &[]).await?;
    if current.snapshot != expected {
        return Err(snapshots::stale());
    }
    // Serialize this author's invite quota across all their projects. The project
    // lock always precedes this lock, and redemption never acquires an author lock.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('envx-project-invites'),hashtext($1))")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE project_invites SET ciphertext=NULL WHERE author_id=$1 AND ciphertext IS NOT NULL AND expires_at<=clock_timestamp()")
        .bind(user).execute(&mut *tx).await?;
    let (count,bytes):(i64,i64)=sqlx::query_as("SELECT count(*),COALESCE(sum(octet_length(ciphertext)),0)::bigint FROM project_invites WHERE author_id=$1 AND invited_id IS NULL AND expires_at>clock_timestamp() AND ciphertext IS NOT NULL")
        .bind(user).fetch_one(&mut *tx).await?;
    if count >= MAX_ACTIVE_INVITES || bytes + body.ciphertext.len() as i64 > MAX_ACTIVE_BYTES {
        return Err(AppError::Generic(
            StatusCode::TOO_MANY_REQUESTS,
            "Active invitation quota exceeded; wait for invitations to expire".into(),
        ));
    }
    let invite_code = sqlx::query_scalar("INSERT INTO project_invites(project_id,author_id,expires_at,verifier_argon2id,ciphertext,snapshot_hash) VALUES($1,$2,clock_timestamp()+interval '1 hour',$3,$4,$5) RETURNING id")
        .bind(body.project_id).bind(user).bind(hash).bind(body.ciphertext).bind(expected).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(InviteResponse {
        invite_code,
        verifier,
    }))
}
