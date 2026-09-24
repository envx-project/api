use super::*;
use pgp::composed::Message;
use rand::RngCore;

pub(super) fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create, list))
        .routes(routes!(revoke))
        .routes(routes!(preview))
        .routes(routes!(redeem))
}
#[derive(Deserialize, ToSchema)]
pub struct CreateLink {
    pub label: String,
    pub target_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, ToSchema)]
pub struct Link {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub target_id: Option<Uuid>,
    pub label: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub redeemed_at: Option<DateTime<Utc>>,
    pub redeemed_by: Option<Identity>,
    pub receipt: Option<String>,
}
#[derive(FromRow)]
struct LinkRow {
    id: Uuid,
    creator_id: Uuid,
    target_id: Option<Uuid>,
    label: String,
    token_hash: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    redeemed_at: Option<DateTime<Utc>>,
    redeemed_by: Option<Uuid>,
    receipt: Option<String>,
}
impl LinkRow {
    pub(super) async fn public(self, pool: &sqlx::PgPool) -> Result<Link, AppError> {
        Ok(Link {
            id: self.id,
            creator_id: self.creator_id,
            target_id: self.target_id,
            label: self.label,
            created_at: self.created_at,
            expires_at: self.expires_at,
            revoked_at: self.revoked_at,
            redeemed_at: self.redeemed_at,
            redeemed_by: match self.redeemed_by {
                Some(id) => Some(identity(pool, id).await?),
                None => None,
            },
            receipt: self.receipt,
        })
    }
    fn validate(&self, user: Uuid, token: &str) -> Result<(), AppError> {
        if token.len() != 64 || hash(token) != self.token_hash {
            return Err(not_found());
        }
        if self.creator_id == user {
            return Err(bad("Cannot redeem your own friend link"));
        }
        if self.target_id.is_some_and(|id| id != user) {
            return Err(not_found());
        }
        // The winning redeemer can recover a lost response even after expiry or removal.
        if self.redeemed_by == Some(user) {
            return Ok(());
        }
        if self.redeemed_by.is_some() {
            return Err(conflict("Link already redeemed"));
        }
        if self.revoked_at.is_some() || self.expires_at <= Utc::now() {
            return Err(conflict("Link expired or revoked"));
        }
        Ok(())
    }
}
#[derive(Serialize, ToSchema)]
pub struct CreatedLink {
    pub link: Link,
    pub token: String,
}
#[derive(Deserialize, ToSchema)]
pub struct Claim {
    pub id: Uuid,
    pub token: String,
}
#[derive(Deserialize, ToSchema)]
pub struct Redeem {
    pub id: Uuid,
    pub token: String,
    pub receipt: String,
}
#[derive(Serialize, ToSchema)]
pub struct LinkPreview {
    pub link: Link,
    pub creator: Identity,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub version: u8,
    pub id: Uuid,
    pub creator_id: Uuid,
    pub creator_fingerprint: String,
    pub redeemer_id: Uuid,
    pub redeemer_fingerprint: String,
}

#[utoipa::path(post,path="/friend-links",tag="friends",request_body=CreateLink,responses((status=200,body=CreatedLink)),security(("bearer"=[])))]
pub(super) async fn create(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<CreateLink>,
) -> Result<Json<CreatedLink>, AppError> {
    if body.label.is_empty()
        || body.label.len() > 64
        || !body
            .label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'-')
    {
        return Err(bad("label must contain 1-64 lowercase letters or hyphens"));
    }
    if body.target_id == Some(user) {
        return Err(bad("Cannot target yourself"));
    }
    let now = Utc::now();
    let expires = body.expires_at.unwrap_or(now + chrono::Duration::hours(24));
    if expires <= now || expires > now + chrono::Duration::days(30) {
        return Err(bad("Link expiry must be in the next 30 days"));
    }
    // Historical registrations only parsed keys; reject unusable identities
    // before publishing an invitation that clients cannot safely accept.
    identity(&state.db, user).await?;
    if let Some(target) = body.target_id {
        identity(&state.db, target).await?;
    }
    let mut random = [0u8; 32];
    rand::rng().fill_bytes(&mut random);
    let token = hex::encode(random);
    let mut tx = state.db.begin().await?;
    lock_users(&mut tx, user, body.target_id.unwrap_or(user)).await?;
    rate(&mut tx, user, "link-create", 100).await?;
    let active: i64=sqlx::query_scalar("SELECT count(*) FROM friend_links WHERE creator_id=$1 AND redeemed_by IS NULL AND revoked_at IS NULL AND expires_at>now()")
        .bind(user).fetch_one(&mut *tx).await?;
    if active >= 100 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many active friend links",
        )
            .into());
    }
    let row:LinkRow=sqlx::query_as("INSERT INTO friend_links(id,creator_id,target_id,label,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,$6) RETURNING *")
        .bind(Uuid::new_v4()).bind(user).bind(body.target_id).bind(body.label).bind(hash(&token)).bind(expires).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(CreatedLink {
        link: row.public(&state.db).await?,
        token,
    }))
}
#[utoipa::path(get,path="/friend-links",tag="friends",params(("before"=Option<Uuid>,Query),("limit"=Option<i64>,Query)),responses((status=200,body=Vec<Link>)),security(("bearer"=[])))]
pub(super) async fn list(
    State(state): State<AppState>,
    UserId(user): UserId,
    Query(page): Query<Page>,
) -> Result<Json<Vec<Link>>, AppError> {
    let limit = page.limit()?;
    let before_time: Option<DateTime<Utc>> = match page.before {
        Some(id) => Some(
            sqlx::query_scalar("SELECT created_at FROM friend_links WHERE id=$1 AND creator_id=$2")
                .bind(id)
                .bind(user)
                .fetch_optional(&*state.db)
                .await?
                .ok_or_else(not_found)?,
        ),
        None => None,
    };
    let rows:Vec<LinkRow>=sqlx::query_as("SELECT * FROM friend_links WHERE creator_id=$1 AND ($2::timestamptz IS NULL OR (created_at,id)<($2,$3)) ORDER BY created_at DESC,id DESC LIMIT $4")
        .bind(user).bind(before_time).bind(page.before).bind(limit).fetch_all(&*state.db).await?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.public(&state.db).await?);
    }
    Ok(Json(result))
}
#[utoipa::path(delete,path="/friend-links/{id}",tag="friends",params(("id"=Uuid,Path)),responses((status=204)),security(("bearer"=[])))]
pub(super) async fn revoke(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result=sqlx::query("UPDATE friend_links SET revoked_at=COALESCE(revoked_at,now()) WHERE id=$1 AND creator_id=$2 AND redeemed_by IS NULL")
        .bind(id).bind(user).execute(&*state.db).await?;
    if result.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
// Commit failed-attempt usage separately: rolling it back would make the cap bypassable.
pub(super) async fn claim_attempt(state: &AppState, user: Uuid) -> Result<(), AppError> {
    let mut tx = state.db.begin().await?;
    rate(&mut tx, user, "link-claim", 1000).await?;
    tx.commit().await?;
    Ok(())
}
#[utoipa::path(post,path="/friend-links/preview",tag="friends",request_body=Claim,responses((status=200,body=LinkPreview)),security(("bearer"=[])))]
pub(super) async fn preview(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<Claim>,
) -> Result<Json<LinkPreview>, AppError> {
    claim_attempt(&state, user).await?;
    let row: LinkRow = sqlx::query_as("SELECT * FROM friend_links WHERE id=$1")
        .bind(body.id)
        .fetch_optional(&*state.db)
        .await?
        .ok_or_else(not_found)?;
    row.validate(user, &body.token)?;
    let creator = identity(&state.db, row.creator_id).await?;
    Ok(Json(LinkPreview {
        link: row.public(&state.db).await?,
        creator,
    }))
}
#[utoipa::path(post,path="/friend-links/redeem",tag="friends",request_body=Redeem,responses((status=200,body=LinkPreview)),security(("bearer"=[])))]
pub(super) async fn redeem(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<Redeem>,
) -> Result<Json<LinkPreview>, AppError> {
    claim_attempt(&state, user).await?;
    if body.receipt.len() > 32 * 1024 {
        return Err(bad("Receipt too large"));
    }
    // Read creator first to preserve the same user-lock order as remove/send.
    let creator_id: Uuid = sqlx::query_scalar("SELECT creator_id FROM friend_links WHERE id=$1")
        .bind(body.id)
        .fetch_optional(&*state.db)
        .await?
        .ok_or_else(not_found)?;
    let creator = identity(&state.db, creator_id).await?;
    let redeemer = identity(&state.db, user).await?;
    let key = SignedPublicKey::from_string(&redeemer.public_key)
        .map_err(|_| bad("Invalid public key"))?
        .0;
    let mut signed = Message::from_string(&body.receipt)
        .map_err(|_| bad("Invalid signed receipt"))?
        .0;
    let data = signed
        .as_data_string()
        .map_err(|_| bad("Invalid receipt payload"))?;
    signed
        .verify(&key)
        .map_err(|_| bad("Invalid receipt signature"))?;
    let receipt: Receipt = serde_json::from_str(&data).map_err(|_| bad("Invalid receipt JSON"))?;
    if receipt.version != 1
        || receipt.id != body.id
        || receipt.creator_id != creator_id
        || receipt.creator_fingerprint != creator.fingerprint
        || receipt.redeemer_id != user
        || receipt.redeemer_fingerprint != redeemer.fingerprint
    {
        return Err(bad("Receipt identity mismatch"));
    }
    let mut tx = state.db.begin().await?;
    lock_users(&mut tx, user, creator_id).await?;
    let row: LinkRow = sqlx::query_as("SELECT * FROM friend_links WHERE id=$1 FOR UPDATE")
        .bind(body.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(not_found)?;
    row.validate(user, &body.token)?;
    if row.redeemed_by == Some(user) {
        tx.commit().await?;
        return Ok(Json(LinkPreview {
            link: row.public(&state.db).await?,
            creator,
        }));
    }
    for account in [user, creator_id] {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM friendships WHERE user_low=$1 OR user_high=$1",
        )
        .bind(account)
        .fetch_one(&mut *tx)
        .await?;
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM friendships WHERE user_low=LEAST($1,$2) AND user_high=GREATEST($1,$2))").bind(user).bind(creator_id).fetch_one(&mut *tx).await?;
        if count >= 1000 && !exists {
            return Err((StatusCode::TOO_MANY_REQUESTS, "Friend limit reached").into());
        }
    }
    sqlx::query("INSERT INTO friendships(user_low,user_high) VALUES(LEAST($1,$2),GREATEST($1,$2)) ON CONFLICT DO NOTHING").bind(user).bind(creator_id).execute(&mut *tx).await?;
    let row:LinkRow=sqlx::query_as("UPDATE friend_links SET redeemed_by=$2,redeemed_at=now(),receipt=$3 WHERE id=$1 RETURNING *").bind(body.id).bind(user).bind(&body.receipt).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(LinkPreview {
        link: row.public(&state.db).await?,
        creator,
    }))
}
