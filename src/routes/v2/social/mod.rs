//! Friend-only handoffs. Locks on user rows serialize pair changes and mailbox quotas.
use crate::{extractors::user::UserId, AppError, AppState, Json, State};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use pgp::{
    composed::{Deserializable, SignedPublicKey},
    types::KeyDetails,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

mod links;
mod messages;
#[cfg(test)]
mod tests;

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(friends, remove_friend))
        .merge(links::router())
        .merge(messages::router())
        .with_state(state)
}

#[derive(Serialize, ToSchema)]
pub struct Identity {
    pub id: Uuid,
    pub username: String,
    pub public_key: String,
    pub fingerprint: String,
}

async fn identity(pool: &sqlx::PgPool, id: Uuid) -> Result<Identity, AppError> {
    let (username, public_key): (String, String) =
        sqlx::query_as("SELECT username, public_key FROM users WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(not_found)?;
    // Legacy registrations predate the smaller registration limit. Bound parsing
    // without rejecting historical keys that still fit the authentication limit.
    if public_key.len() > 1024 * 1024 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid stored public key",
        )
            .into());
    }
    let key = SignedPublicKey::from_string(&public_key)
        .map_err(|_| {
            AppError::Generic(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid stored public key".into(),
            )
        })?
        .0;
    Ok(Identity {
        id,
        username,
        fingerprint: hex::encode(key.fingerprint().as_bytes()),
        public_key,
    })
}

fn not_found() -> AppError {
    (StatusCode::NOT_FOUND, "Not found").into()
}
fn bad(message: &str) -> AppError {
    (StatusCode::BAD_REQUEST, message.to_string()).into()
}
fn conflict(message: &str) -> AppError {
    (StatusCode::CONFLICT, message.to_string()).into()
}
fn hash(value: &str) -> String {
    crypto_hash::hex_digest(crypto_hash::Algorithm::SHA256, value.as_bytes())
}

async fn lock_users(tx: &mut Transaction<'_, Postgres>, a: Uuid, b: Uuid) -> Result<(), AppError> {
    let rows: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 OR id=$2 ORDER BY id FOR UPDATE")
            .bind(a)
            .bind(b)
            .fetch_all(&mut **tx)
            .await?;
    if rows.len() != if a == b { 1 } else { 2 } {
        return Err(not_found());
    }
    Ok(())
}
async fn rate(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    kind: &str,
    cap: i64,
) -> Result<(), AppError> {
    let count: Option<i64> = sqlx::query_scalar("INSERT INTO social_daily_usage(user_id,kind,count) VALUES($1,$2,1) ON CONFLICT(user_id,day,kind) DO UPDATE SET count=social_daily_usage.count+1 WHERE social_daily_usage.count<$3 RETURNING count")
        .bind(user).bind(kind).bind(cap).fetch_optional(&mut **tx).await?;
    if count.is_none() {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Daily limit reached").into());
    }
    Ok(())
}

#[derive(Deserialize, ToSchema, Default)]
pub struct Page {
    pub before: Option<Uuid>,
    pub limit: Option<i64>,
}
impl Page {
    fn limit(&self) -> Result<i64, AppError> {
        let value = self.limit.unwrap_or(50);
        if !(1..=100).contains(&value) {
            return Err(bad("limit must be between 1 and 100"));
        }
        Ok(value)
    }
}
#[derive(Serialize, ToSchema)]
pub struct Friend {
    pub user: Identity,
    pub created_at: DateTime<Utc>,
    /// Signed redemption evidence; verify locally before accepting a new key.
    pub receipts: Vec<String>,
}

#[utoipa::path(get, path="/friends", tag="friends", params(("before"=Option<Uuid>,Query),("limit"=Option<i64>,Query)), responses((status=200,body=Vec<Friend>)), security(("bearer"=[])))]
async fn friends(
    State(state): State<AppState>,
    UserId(user): UserId,
    Query(page): Query<Page>,
) -> Result<Json<Vec<Friend>>, AppError> {
    let rows: Vec<(Uuid, DateTime<Utc>)> = sqlx::query_as("SELECT CASE WHEN user_low=$1 THEN user_high ELSE user_low END AS friend_id,created_at FROM friendships WHERE (user_low=$1 OR user_high=$1) AND ($2::uuid IS NULL OR (CASE WHEN user_low=$1 THEN user_high ELSE user_low END)<$2) ORDER BY friend_id DESC LIMIT $3")
        .bind(user).bind(page.before).bind(page.limit()?).fetch_all(&*state.db).await?;
    let mut result = Vec::new();
    for (friend_id, created_at) in rows {
        let receipts: Vec<String>=sqlx::query_scalar("SELECT receipt FROM friend_links WHERE ((creator_id=$1 AND redeemed_by=$2) OR (creator_id=$2 AND redeemed_by=$1)) AND receipt IS NOT NULL ORDER BY redeemed_at DESC LIMIT 10")
            .bind(user).bind(friend_id).fetch_all(&*state.db).await?;
        result.push(Friend {
            user: identity(&state.db, friend_id).await?,
            created_at,
            receipts,
        });
    }
    Ok(Json(result))
}

#[utoipa::path(delete, path="/friends/{user_id}", tag="friends", params(("user_id"=Uuid,Path)), responses((status=204)), security(("bearer"=[])))]
async fn remove_friend(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(friend): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mut tx = state.db.begin().await?;
    lock_users(&mut tx, user, friend).await?;
    sqlx::query(
        "DELETE FROM friendships WHERE user_low=LEAST($1,$2) AND user_high=GREATEST($1,$2)",
    )
    .bind(user)
    .bind(friend)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
