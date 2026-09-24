use super::*;
use pgp::composed::ArmorOptions;
pub(super) fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(send, list))
        .routes(routes!(get, delete))
}
#[derive(Deserialize, ToSchema)]
pub struct SendMessage {
    pub id: Uuid,
    pub recipient_id: Uuid,
    pub ciphertext: String,
    pub expires_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, FromRow, ToSchema)]
pub struct SecretMessage {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub sender_public_key: String,
    pub recipient_public_key: String,
    pub ciphertext: Option<String>,
}
#[derive(FromRow)]
struct StoredMessage {
    id: Uuid,
    sender_id: Uuid,
    recipient_id: Uuid,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    sender_public_key: String,
    recipient_public_key: String,
    ciphertext: Option<String>,
    ciphertext_hash: String,
    sender_deleted: bool,
    recipient_deleted: bool,
}
impl StoredMessage {
    fn public(self, include_ciphertext: bool) -> SecretMessage {
        SecretMessage {
            id: self.id,
            sender_id: self.sender_id,
            recipient_id: self.recipient_id,
            created_at: self.created_at,
            expires_at: self.expires_at,
            sender_public_key: if include_ciphertext {
                self.sender_public_key
            } else {
                String::new()
            },
            recipient_public_key: if include_ciphertext {
                self.recipient_public_key
            } else {
                String::new()
            },
            ciphertext: if include_ciphertext {
                self.ciphertext
            } else {
                None
            },
        }
    }
    fn visible(&self, user: Uuid) -> bool {
        self.expires_at.is_none_or(|at| at > Utc::now())
            && self.ciphertext.is_some()
            && ((self.sender_id == user && !self.sender_deleted)
                || (self.recipient_id == user && !self.recipient_deleted))
    }
}
#[utoipa::path(post,path="/messages",tag="messages",request_body=SendMessage,responses((status=200,body=SecretMessage)),security(("bearer"=[])))]
pub(super) async fn send(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(mut body): Json<SendMessage>,
) -> Result<Json<SecretMessage>, AppError> {
    // PostgreSQL timestamps have microsecond precision; normalize before storage.
    body.expires_at = body
        .expires_at
        .and_then(|at| DateTime::from_timestamp_micros(at.timestamp_micros()));
    if body.recipient_id == user {
        return Err(bad("Cannot send to yourself"));
    }
    if body.ciphertext.is_empty() || body.ciphertext.len() > 128 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Ciphertext must contain 1-131072 bytes",
        )
            .into());
    }
    // Opportunistic bounded cleanup; expiry access checks do not depend on cleanup.
    sqlx::query("WITH expired AS (SELECT id FROM secret_messages WHERE expires_at<=now() AND ciphertext IS NOT NULL ORDER BY expires_at LIMIT 1000 FOR UPDATE SKIP LOCKED) UPDATE secret_messages SET ciphertext=NULL,sender_public_key='',recipient_public_key='' FROM expired WHERE secret_messages.id=expired.id")
        .execute(&*state.db).await?;
    let mut tx = state.db.begin().await?;
    lock_users(&mut tx, user, body.recipient_id).await?;
    // Retain the digest after deletion so a retried request cannot resurrect a secret.
    let existing: Option<StoredMessage> =
        sqlx::query_as("SELECT * FROM secret_messages WHERE id=$1")
            .bind(body.id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(existing) = existing {
        if existing.sender_id != user
            || existing.recipient_id != body.recipient_id
            || existing.ciphertext_hash != hash(&body.ciphertext)
            || existing.expires_at.map(|at| at.timestamp_micros())
                != body.expires_at.map(|at| at.timestamp_micros())
        {
            return Err(conflict("Message ID already used"));
        }
        tx.commit().await?;
        return Ok(Json(existing.public(false)));
    }
    if body.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
        return Err(bad("Message expiry must be in the future"));
    }
    let friends:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM friendships WHERE user_low=LEAST($1,$2) AND user_high=GREATEST($1,$2))").bind(user).bind(body.recipient_id).fetch_one(&mut *tx).await?;
    if !friends {
        return Err((StatusCode::FORBIDDEN, "Recipient must be a friend").into());
    }
    let mut keys = Vec::new();
    for account in [user, body.recipient_id] {
        // Legacy registrations predate key-size caps. Do not parse unbounded armor.
        let raw: Option<String> = sqlx::query_scalar("SELECT CASE WHEN octet_length(public_key)<=1048576 THEN public_key ELSE NULL END FROM users WHERE id=$1")
            .bind(account).fetch_one(&mut *tx).await?;
        keys.push(canonical_key(
            raw.as_deref()
                .ok_or_else(|| bad("Stored public key exceeds size limit"))?,
        )?);
    }
    let message_bytes = (body.ciphertext.len() + keys[0].len() + keys[1].len()) as i64;
    rate(&mut tx, user, "message-send", 1000).await?;
    for account in [user, body.recipient_id] {
        let (count,bytes):(i64,i64)=sqlx::query_as("SELECT count(*),COALESCE(sum(octet_length(ciphertext)+octet_length(sender_public_key)+octet_length(recipient_public_key)),0)::bigint FROM secret_messages WHERE ((sender_id=$1 AND NOT sender_deleted) OR (recipient_id=$1 AND NOT recipient_deleted)) AND (expires_at IS NULL OR expires_at>now()) AND ciphertext IS NOT NULL")
            .bind(account).fetch_one(&mut *tx).await?;
        if count >= 1000 || bytes + message_bytes > 20 * 1024 * 1024 {
            return Err((StatusCode::TOO_MANY_REQUESTS, "Mailbox quota exceeded").into());
        }
    }
    let row:StoredMessage=sqlx::query_as("INSERT INTO secret_messages(id,sender_id,recipient_id,ciphertext,ciphertext_hash,sender_public_key,recipient_public_key,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
        .bind(body.id).bind(user).bind(body.recipient_id).bind(&body.ciphertext).bind(hash(&body.ciphertext)).bind(&keys[0]).bind(&keys[1]).bind(body.expires_at).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(row.public(false)))
}
#[utoipa::path(get,path="/messages",tag="messages",params(("before"=Option<Uuid>,Query),("limit"=Option<i64>,Query)),responses((status=200,body=Vec<SecretMessage>)),security(("bearer"=[])))]
pub(super) async fn list(
    State(state): State<AppState>,
    UserId(user): UserId,
    Query(page): Query<Page>,
) -> Result<Json<Vec<SecretMessage>>, AppError> {
    let limit = page.limit()?;
    // A previously visible cursor remains usable after deletion or expiry, but
    // another account's message cannot be used to probe its creation time.
    let before_time: Option<DateTime<Utc>> = match page.before {
        Some(id) => Some(sqlx::query_scalar("SELECT created_at FROM secret_messages WHERE id=$1 AND (sender_id=$2 OR recipient_id=$2)")
            .bind(id).bind(user).fetch_optional(&*state.db).await?.ok_or_else(not_found)?),
        None => None,
    };
    let rows:Vec<SecretMessage>=sqlx::query_as("SELECT id,sender_id,recipient_id,created_at,expires_at,''::text AS sender_public_key,''::text AS recipient_public_key,NULL::text AS ciphertext FROM secret_messages WHERE ((sender_id=$1 AND NOT sender_deleted) OR (recipient_id=$1 AND NOT recipient_deleted)) AND (expires_at IS NULL OR expires_at>now()) AND ciphertext IS NOT NULL AND ($2::timestamptz IS NULL OR (created_at,id)<($2,$3)) ORDER BY created_at DESC,id DESC LIMIT $4")
        .bind(user).bind(before_time).bind(page.before).bind(limit).fetch_all(&*state.db).await?;
    Ok(Json(rows))
}
#[utoipa::path(get,path="/messages/{id}",tag="messages",params(("id"=Uuid,Path)),responses((status=200,body=SecretMessage)),security(("bearer"=[])))]
pub(super) async fn get(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<Uuid>,
) -> Result<Json<SecretMessage>, AppError> {
    let row: StoredMessage = sqlx::query_as(
        "SELECT * FROM secret_messages WHERE id=$1 AND (sender_id=$2 OR recipient_id=$2)",
    )
    .bind(id)
    .bind(user)
    .fetch_optional(&*state.db)
    .await?
    .ok_or_else(not_found)?;
    if !row.visible(user) {
        return Err(not_found());
    }
    Ok(Json(row.public(true)))
}
#[utoipa::path(delete,path="/messages/{id}",tag="messages",params(("id"=Uuid,Path)),responses((status=204)),security(("bearer"=[])))]
pub(super) async fn delete(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result=sqlx::query("UPDATE secret_messages SET sender_deleted=sender_deleted OR sender_id=$2,recipient_deleted=recipient_deleted OR recipient_id=$2,ciphertext=CASE WHEN (sender_deleted OR sender_id=$2) AND (recipient_deleted OR recipient_id=$2) THEN NULL ELSE ciphertext END,sender_public_key=CASE WHEN (sender_deleted OR sender_id=$2) AND (recipient_deleted OR recipient_id=$2) THEN '' ELSE sender_public_key END,recipient_public_key=CASE WHEN (sender_deleted OR sender_id=$2) AND (recipient_deleted OR recipient_id=$2) THEN '' ELSE recipient_public_key END WHERE id=$1 AND (sender_id=$2 OR recipient_id=$2)")
        .bind(id).bind(user).execute(&*state.db).await?;
    if result.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

fn canonical_key(raw: &str) -> Result<String, AppError> {
    let key = SignedPublicKey::from_string(raw)
        .map_err(|_| bad("Invalid stored public key"))?
        .0;
    key.verify()
        .map_err(|_| bad("Invalid stored public key signatures"))?;
    let canonical = key
        .to_armored_string(ArmorOptions::default())
        .map_err(|_| bad("Invalid stored public key"))?;
    if canonical.len() > 128 * 1024 {
        return Err(bad("Canonical public key exceeds size limit"));
    }
    Ok(canonical)
}
