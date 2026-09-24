//! A project row is the serialization lock for every membership and variable writer.
//! Snapshot digests bind encrypted row identities/content and all recipient keys.
use crate::{error::Errors, helpers::caps, AppError, AppState};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use std::{collections::HashSet, net::IpAddr};
use utoipa::ToSchema;
use uuid::Uuid;

pub const VERSION: u8 = 1;
pub fn conflict(code: &'static str, message: &'static str) -> AppError {
    AppError::Protocol(StatusCode::CONFLICT, code, message)
}
pub fn upgrade() -> AppError {
    conflict(
        "invite_upgrade_required",
        "Upgrade envx and regenerate this invitation using a project snapshot",
    )
}
pub fn stale() -> AppError {
    conflict(
        "project_snapshot_stale",
        "Project or recipients changed; fetch a fresh snapshot and regenerate the invitation",
    )
}
pub fn require_version(version: Option<u8>) -> Result<(), AppError> {
    if version != Some(VERSION) {
        return Err(upgrade());
    }
    Ok(())
}
pub async fn lock_project(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
) -> Result<(), AppError> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(project)
        .fetch_optional(&mut **tx)
        .await?;
    if found.is_none() {
        return Err(Errors::NotFound.into());
    }
    Ok(())
}
pub async fn authorize(
    connection: &mut PgConnection,
    project: Uuid,
    user: Uuid,
) -> Result<(), AppError> {
    let member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_project_relations WHERE project_id=$1 AND user_id=$2)",
    )
    .bind(project)
    .bind(user)
    .fetch_one(connection)
    .await?;
    if !member {
        return Err(Errors::Unauthorized.into());
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct SnapshotVariable {
    pub id: Uuid,
    pub project_id: Uuid,
    pub value: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tag: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Recipient {
    pub id: Uuid,
    pub public_key: String,
}
#[derive(Serialize, ToSchema)]
pub struct Snapshot {
    pub protocol_version: u8,
    pub project_id: Uuid,
    pub snapshot: String,
    pub variables: Vec<SnapshotVariable>,
    pub users: Vec<Recipient>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct RewrappedVariable {
    pub id: Uuid,
    pub value: String,
}

/// Call only while holding the project lock. Added recipients are part of the token,
/// so a caller cannot re-use ciphertext prepared for another set of public keys.
pub async fn read(
    connection: &mut PgConnection,
    project: Uuid,
    added: &[Uuid],
) -> Result<Snapshot, AppError> {
    let variables: Vec<SnapshotVariable> = sqlx::query_as(
        "SELECT id,project_id,value,created_at,tag FROM variables WHERE project_id=$1 ORDER BY id",
    )
    .bind(project)
    .fetch_all(&mut *connection)
    .await?;
    let members: Vec<Recipient> = sqlx::query_as("SELECT u.id,u.public_key FROM users u JOIN user_project_relations r ON r.user_id=u.id WHERE r.project_id=$1 ORDER BY u.id")
        .bind(project).fetch_all(&mut *connection).await?;
    let mut users = members.clone();
    let added_users: Vec<Recipient> =
        sqlx::query_as("SELECT id,public_key FROM users WHERE id=ANY($1) ORDER BY id")
            .bind(added)
            .fetch_all(&mut *connection)
            .await?;
    if added_users.len() != added.iter().collect::<HashSet<_>>().len() {
        return Err(AppError::Generic(
            StatusCode::BAD_REQUEST,
            "Unknown recipient".into(),
        ));
    }
    users.extend(added_users);
    users.sort_by_key(|user| user.id);
    users.dedup_by_key(|user| user.id);
    let encoded = serde_json::to_vec(&(
        "envx-project-snapshot-v1",
        project,
        &variables,
        &members,
        &users,
    ))
    .map_err(anyhow::Error::from)?;
    let snapshot = crypto_hash::hex_digest(crypto_hash::Algorithm::SHA256, &encoded);
    Ok(Snapshot {
        protocol_version: VERSION,
        project_id: project,
        snapshot,
        variables,
        users,
    })
}

pub async fn rewrap(
    state: &AppState,
    connection: &mut PgConnection,
    current: &Snapshot,
    expected: &str,
    variables: &[RewrappedVariable],
    user: Uuid,
    ip: IpAddr,
) -> Result<(), AppError> {
    if current.snapshot != expected {
        return Err(stale());
    }
    let actual: HashSet<_> = variables.iter().map(|v| v.id).collect();
    let required: HashSet<_> = current.variables.iter().map(|v| v.id).collect();
    if actual != required || actual.len() != variables.len() {
        return Err(AppError::Generic(
            StatusCode::BAD_REQUEST,
            "Rewrap must contain every snapshot variable exactly once".into(),
        ));
    }
    let values: Vec<&str> = variables.iter().map(|v| v.value.as_str()).collect();
    caps::check_per_value(&state.caps, &values)?;
    let ids: Vec<Uuid> = variables.iter().map(|v| v.id).collect();
    caps::check_project_for_update_on(&state.caps, connection, current.project_id, &ids, &values)
        .await?;
    let new_bytes: i64 = values.iter().map(|v| v.len() as i64).sum();
    let old_bytes: i64 = current.variables.iter().map(|v| v.value.len() as i64).sum();
    // Invitees are not members yet, so their storage delta is the entire project.
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_project_relations WHERE project_id=$1 AND user_id=$2)",
    )
    .bind(current.project_id)
    .bind(user)
    .fetch_one(&mut *connection)
    .await?;
    caps::check_user_total_on(
        &state.caps,
        connection,
        user,
        if is_member {
            new_bytes - old_bytes
        } else {
            new_bytes
        },
    )
    .await?;
    caps::check_and_record_ip_on(&state.caps, connection, ip, (new_bytes - old_bytes).max(0))
        .await?;
    for variable in variables {
        sqlx::query("UPDATE variables SET value=$1 WHERE id=$2 AND project_id=$3")
            .bind(&variable.value)
            .bind(variable.id)
            .bind(current.project_id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

pub async fn remove_members(
    state: &AppState,
    project: Uuid,
    user: Uuid,
    removed: &[Uuid],
) -> Result<(), AppError> {
    let mut tx = state.db.begin().await?;
    lock_project(&mut tx, project).await?;
    authorize(&mut tx, project, user).await?;
    sqlx::query("DELETE FROM user_project_relations WHERE project_id=$1 AND user_id=ANY($2)")
        .bind(project)
        .bind(removed)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
