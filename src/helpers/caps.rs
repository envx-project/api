//! Enforce abuse-prevention caps before a write lands. All checks are
//! soft — concurrent writes between the check and the insert can race
//! past the cap by a small margin. That's acceptable for abuse defense;
//! the absolute upper bound the user can sustain is bounded by these
//! checks even if a single batch slips through.

use std::collections::HashMap;
use std::net::IpAddr;

use axum::http::StatusCode;
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::types::Uuid;

use crate::config::Caps;
use crate::error::AppError;
use crate::state::{AppState, DB};
use crate::traits::to_uuid::ToUuid;
use crate::Context as _;

fn too_big(msg: String) -> AppError {
    AppError::Generic(StatusCode::BAD_REQUEST, msg)
}

fn too_many(msg: String) -> AppError {
    AppError::Generic(StatusCode::PAYLOAD_TOO_LARGE, msg)
}

fn rate_limited(msg: String) -> AppError {
    AppError::Generic(StatusCode::TOO_MANY_REQUESTS, msg)
}

/// Reject any single value larger than the per-value cap.
pub fn check_per_value(caps: &Caps, values: &[&str]) -> Result<(), AppError> {
    if caps.max_variable_bytes == 0 {
        return Ok(());
    }
    let max = caps.max_variable_bytes as usize;
    for v in values {
        if v.len() > max {
            return Err(too_big(format!(
                "variable exceeds max size: {} bytes (cap {})",
                v.len(),
                max,
            )));
        }
    }
    Ok(())
}

/// Reject an INSERT that would push the project past either the count
/// cap or the byte cap.
pub async fn check_project_for_insert(
    caps: &Caps,
    db: &DB,
    project_id: Uuid,
    new_values: &[&str],
) -> Result<(), AppError> {
    let count_cap = caps.max_variables_per_project;
    let bytes_cap = caps.max_project_bytes;
    if count_cap == 0 && bytes_cap == 0 {
        return Ok(());
    }

    let row = sqlx::query!(
        r#"SELECT
              count(*) AS "count!",
              COALESCE(sum(octet_length(value))::bigint, 0) AS "bytes!"
           FROM variables WHERE project_id = $1"#,
        project_id
    )
    .fetch_one(db.as_ref())
    .await
    .context("Failed to fetch project size")?;

    let new_count = new_values.len() as i64;
    let new_bytes: i64 = new_values.iter().map(|v| v.len() as i64).sum();

    if count_cap > 0 && row.count + new_count > count_cap {
        return Err(too_many(format!(
            "project would exceed variable cap: {} (cap {})",
            row.count + new_count,
            count_cap,
        )));
    }
    if bytes_cap > 0 && row.bytes + new_bytes > bytes_cap {
        return Err(too_many(format!(
            "project would exceed size cap: {} bytes (cap {})",
            row.bytes + new_bytes,
            bytes_cap,
        )));
    }
    Ok(())
}

/// Reject an UPDATE that would push the project past the byte cap.
/// (Count does not change on update.)
pub async fn check_project_for_update(
    caps: &Caps,
    db: &DB,
    project_id: Uuid,
    update_ids: &[Uuid],
    new_values: &[&str],
) -> Result<(), AppError> {
    let bytes_cap = caps.max_project_bytes;
    if bytes_cap == 0 {
        return Ok(());
    }

    let row = sqlx::query!(
        r#"SELECT
              COALESCE(sum(CASE WHEN id = ANY($2::uuid[])
                                THEN octet_length(value) ELSE 0 END)::bigint, 0) AS "replaced!",
              COALESCE(sum(octet_length(value))::bigint, 0) AS "total!"
           FROM variables WHERE project_id = $1"#,
        project_id,
        update_ids
    )
    .fetch_one(db.as_ref())
    .await
    .context("Failed to fetch project size")?;

    let new_bytes: i64 = new_values.iter().map(|v| v.len() as i64).sum();
    let final_bytes = row.total - row.replaced + new_bytes;

    if final_bytes > bytes_cap {
        return Err(too_many(format!(
            "project would exceed size cap: {} bytes (cap {})",
            final_bytes, bytes_cap,
        )));
    }
    Ok(())
}

/// Reject a write that would push the user's total across all of their
/// projects past the per-user cap. `delta_bytes` is the net bytes the
/// pending write would add (new bytes minus replaced bytes; may be
/// negative for updates that shrink values).
pub async fn check_user_total(
    caps: &Caps,
    db: &DB,
    user_id: Uuid,
    delta_bytes: i64,
) -> Result<(), AppError> {
    let cap = caps.max_user_bytes;
    if cap == 0 {
        return Ok(());
    }

    let row = sqlx::query!(
        r#"SELECT COALESCE(sum(octet_length(v.value))::bigint, 0) AS "bytes!"
           FROM variables v
           JOIN user_project_relations upr ON v.project_id = upr.project_id
           WHERE upr.user_id = $1"#,
        user_id
    )
    .fetch_one(db.as_ref())
    .await
    .context("Failed to fetch user total")?;

    let final_bytes = row.bytes + delta_bytes;
    if final_bytes > cap {
        return Err(too_many(format!(
            "user storage cap exceeded: {} bytes across your projects (cap {})",
            final_bytes, cap,
        )));
    }
    Ok(())
}

/// Reject a write from an IP that has uploaded more than the per-IP cap
/// in the last 24 hours, then record this write's byte count in the
/// upload_log table. Opportunistically prunes rows older than 24h.
pub async fn check_and_record_ip(
    caps: &Caps,
    db: &DB,
    ip: IpAddr,
    bytes: i64,
) -> Result<(), AppError> {
    let cap = caps.max_ip_bytes_per_day;
    if cap == 0 {
        return Ok(());
    }

    let net: IpNetwork = ip.into();

    // Prune old rows opportunistically. Cheap when the index is hot.
    sqlx::query!("DELETE FROM upload_log WHERE created_at < now() - interval '24 hours'")
        .execute(db.as_ref())
        .await
        .context("Failed to prune upload_log")?;

    let row = sqlx::query!(
        r#"SELECT COALESCE(sum(bytes)::bigint, 0) AS "bytes!"
           FROM upload_log
           WHERE ip = $1 AND created_at > now() - interval '24 hours'"#,
        net
    )
    .fetch_one(db.as_ref())
    .await
    .context("Failed to fetch ip upload total")?;

    if row.bytes + bytes > cap {
        return Err(rate_limited(format!(
            "rate limit: this IP has uploaded {} bytes in the last 24 hours (cap {})",
            row.bytes + bytes,
            cap,
        )));
    }

    sqlx::query!(
        "INSERT INTO upload_log (ip, bytes) VALUES ($1, $2)",
        net,
        bytes
    )
    .execute(db.as_ref())
    .await
    .context("Failed to record upload_log entry")?;

    Ok(())
}

/// Cross-project update-many helper: groups the incoming updates by
/// project, runs the per-project byte check on each, then runs the
/// per-user and per-IP checks on the aggregate delta. `triples` is
/// `(project_id_str, variable_id_str, new_value)`.
pub async fn check_update_caps_grouped(
    state: &AppState,
    user_id: Uuid,
    ip: IpAddr,
    triples: Vec<(&str, &str, &str)>,
) -> Result<(), AppError> {
    let mut by_project: HashMap<Uuid, (Vec<Uuid>, Vec<&str>)> = HashMap::new();
    let mut total_new_bytes: i64 = 0;

    for (pid_str, vid_str, value) in &triples {
        let pid = pid_str.to_string().to_uuid()?;
        let vid = vid_str.to_string().to_uuid()?;
        let entry = by_project.entry(pid).or_default();
        entry.0.push(vid);
        entry.1.push(*value);
        total_new_bytes += value.len() as i64;
    }

    let mut total_replaced_bytes: i64 = 0;
    for (pid, (ids, values)) in &by_project {
        check_project_for_update(&state.caps, &state.db, *pid, ids, values).await?;

        let row = sqlx::query!(
            r#"SELECT COALESCE(sum(octet_length(value))::bigint, 0) AS "bytes!"
               FROM variables WHERE id = ANY($1::uuid[])"#,
            ids
        )
        .fetch_one(state.db.as_ref())
        .await
        .context("Failed to fetch replaced byte count")?;
        total_replaced_bytes += row.bytes;
    }

    let delta = total_new_bytes - total_replaced_bytes;
    check_user_total(&state.caps, &state.db, user_id, delta).await?;
    check_and_record_ip(&state.caps, &state.db, ip, delta.max(0)).await?;

    Ok(())
}
