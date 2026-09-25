//! Bounded relay for client-authenticated Noise pairing. The server cannot approve a peer.
use crate::{
    extractors::{client_ip::ClientIp, user::UserId},
    AppError, AppState, Json, State,
};
use axum::{extract::Path, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create))
        .routes(routes!(source))
        .routes(routes!(receiver))
        .with_state(state)
}

pub async fn cleanup(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM auth_pairings WHERE expires_at <= clock_timestamp()")
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Serialize, ToSchema)]
struct Created {
    id: Uuid,
}
#[derive(Deserialize, ToSchema)]
struct Exchange {
    action: String,
    token: Option<String>,
    message: Option<String>,
}
#[derive(Serialize, ToSchema)]
struct Reply {
    phase: i16,
    message: Option<String>,
}
#[derive(sqlx::FromRow)]
struct Session {
    owner_id: Uuid,
    phase: i16,
    receiver_hash: Option<String>,
    message: Option<String>,
}
fn invalid() -> AppError {
    (
        StatusCode::CONFLICT,
        "Pairing unavailable or invalid transition",
    )
        .into()
}
fn hash(token: &str) -> String {
    crypto_hash::hex_digest(crypto_hash::Algorithm::SHA256, token.as_bytes())
}
fn valid_hex(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes * 2
        && value.len().is_multiple_of(2)
        && value.bytes().all(|c| c.is_ascii_hexdigit())
}

#[utoipa::path(post,path="/new",tag="auth pairing",responses((status=200,body=Created)),security(("bearer"=[])))]
async fn create(
    State(state): State<AppState>,
    UserId(owner): UserId,
    ClientIp(ip): ClientIp,
) -> Result<Json<Created>, AppError> {
    let mut tx = state.db.begin().await?;
    // Serialize creation to enforce both per-account and global storage bounds.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('envx-auth-pairing-quota'))")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM auth_pairings WHERE expires_at<=clock_timestamp()")
        .execute(&mut *tx)
        .await?;
    let (total, own, client): (i64, i64, i64) =
        sqlx::query_as("SELECT count(*),count(*) FILTER (WHERE owner_id=$1),count(*) FILTER (WHERE client_network=network(set_masklen($2::inet,CASE WHEN family($2::inet)=6 THEN 64 ELSE 32 END))) FROM auth_pairings")
            .bind(owner)
            .bind(ip.to_string())
            .fetch_one(&mut *tx)
            .await?;
    if own >= 3 || client >= 12 || total >= 1024 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Pairing capacity reached; cancel a pairing or wait for expiry",
        )
            .into());
    }
    let id = sqlx::query_scalar("INSERT INTO auth_pairings(owner_id,client_network) VALUES($1,network(set_masklen($2::inet,CASE WHEN family($2::inet)=6 THEN 64 ELSE 32 END))) RETURNING id")
        .bind(owner)
        .bind(ip.to_string())
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Created { id }))
}

#[utoipa::path(post,path="/{id}/source",tag="auth pairing",params(("id"=Uuid,Path)),request_body=Exchange,responses((status=200,body=Reply)),security(("bearer"=[])))]
async fn source(
    State(state): State<AppState>,
    UserId(owner): UserId,
    Path(id): Path<Uuid>,
    Json(body): Json<Exchange>,
) -> Result<Json<Reply>, AppError> {
    exchange(&state, id, Some(owner), body).await
}
#[utoipa::path(post,path="/{id}/receiver",tag="auth pairing",params(("id"=Uuid,Path)),request_body=Exchange,responses((status=200,body=Reply)))]
async fn receiver(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Exchange>,
) -> Result<Json<Reply>, AppError> {
    exchange(&state, id, None, body).await
}
async fn exchange(
    state: &AppState,
    id: Uuid,
    owner: Option<Uuid>,
    body: Exchange,
) -> Result<Json<Reply>, AppError> {
    let token_hash = if owner.is_none() {
        let token = body.token.as_deref().ok_or_else(invalid)?;
        if token.len() != 64 || !valid_hex(token, 32) {
            return Err(invalid());
        }
        Some(hash(token))
    } else {
        None
    };
    let mut tx = state.db.begin().await?;
    let session:Session=sqlx::query_as("SELECT owner_id,phase,receiver_hash,message FROM auth_pairings WHERE id=$1 AND expires_at>clock_timestamp() FOR UPDATE")
        .bind(id).fetch_optional(&mut *tx).await?.ok_or_else(invalid)?;
    if let Some(owner) = owner {
        if owner != session.owner_id {
            return Err(invalid());
        }
    } else if body.action != "claim" && token_hash != session.receiver_hash {
        return Err(invalid());
    }
    let source = owner.is_some();
    let next = match (source, body.action.as_str(), session.phase) {
        (_, "poll", phase) => {
            let visible = matches!((source, phase), (true, 1 | 3) | (false, 2 | 4));
            tx.commit().await?;
            return Ok(Json(Reply {
                phase,
                message: if visible { session.message } else { None },
            }));
        }
        (false, "claim", 0) => 1,
        (true, "handshake", 1) => 2,
        (false, "handshake", 2) => 3,
        (true, "payload", 3) => 4,
        (_, "cancel", _) | (false, "ack", 4) => {
            sqlx::query("DELETE FROM auth_pairings WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(Json(Reply {
                phase: 5,
                message: None,
            }));
        }
        _ => return Err(invalid()),
    };
    let message = body.message.as_deref().ok_or_else(invalid)?;
    if !valid_hex(message, if next == 4 { 65535 } else { 1024 }) {
        return Err(invalid());
    }
    sqlx::query("UPDATE auth_pairings SET phase=$2,message=$3,receiver_hash=COALESCE(receiver_hash,$4) WHERE id=$1")
        .bind(id).bind(next).bind(message).bind(token_hash).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(Reply {
        phase: next,
        message: None,
    }))
}
