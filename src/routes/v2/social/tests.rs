use super::*;
use pgp::{
    composed::{ArmorOptions, KeyType, MessageBuilder, SecretKeyParamsBuilder, SignedSecretKey},
    crypto::hash::HashAlgorithm,
};
use std::sync::Arc;

fn state(pool: sqlx::PgPool) -> AppState {
    AppState {
        db: Arc::new(pool),
        caps: Arc::new(crate::config::Caps::from_env()),
    }
}
async fn user(pool: &sqlx::PgPool) -> (Uuid, SignedSecretKey) {
    let key = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_sign(true)
        .primary_user_id("test".into())
        .build()
        .unwrap()
        .generate(rand_08::rngs::OsRng)
        .unwrap()
        .sign(rand_08::rngs::OsRng, &"".into())
        .unwrap();
    let public = SignedPublicKey::from(key.clone())
        .to_armored_string(ArmorOptions::default())
        .unwrap();
    let id =
        sqlx::query_scalar("INSERT INTO users(username,public_key) VALUES('test',$1) RETURNING id")
            .bind(public)
            .fetch_one(pool)
            .await
            .unwrap();
    (id, key)
}
fn ok<T>(result: Result<T, AppError>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            use axum::response::IntoResponse;
            panic!("request failed: {}", e.into_response().status())
        }
    }
}
async fn link(pool: &sqlx::PgPool, owner: Uuid, target: Option<Uuid>) -> links::CreatedLink {
    ok(links::create(
        State(state(pool.clone())),
        UserId(owner),
        Json(links::CreateLink {
            label: "amber-otter".into(),
            target_id: target,
            expires_at: None,
        }),
    )
    .await)
    .0
}
async fn receipt(
    pool: &sqlx::PgPool,
    link: &links::CreatedLink,
    id: Uuid,
    key: &SignedSecretKey,
) -> String {
    let creator = ok(identity(pool, link.link.creator_id).await);
    let redeemer = ok(identity(pool, id).await);
    let payload = links::Receipt {
        version: 1,
        id: link.link.id,
        creator_id: creator.id,
        creator_fingerprint: creator.fingerprint,
        redeemer_id: id,
        redeemer_fingerprint: redeemer.fingerprint,
    };
    sign(key, &serde_json::to_string(&payload).unwrap())
}
fn sign(key: &SignedSecretKey, data: &str) -> String {
    let mut builder = MessageBuilder::from_bytes("", data.as_bytes().to_vec());
    builder.sign(&key.primary_key, "".into(), HashAlgorithm::Sha256);
    builder
        .to_armored_string(rand_08::rngs::OsRng, ArmorOptions::default())
        .unwrap()
}
async fn redeem(
    pool: &sqlx::PgPool,
    link: &links::CreatedLink,
    id: Uuid,
    key: &SignedSecretKey,
) -> Result<Json<links::LinkPreview>, AppError> {
    links::redeem(
        State(state(pool.clone())),
        UserId(id),
        Json(links::Redeem {
            id: link.link.id,
            token: link.token.clone(),
            receipt: receipt(pool, link, id, key).await,
        }),
    )
    .await
}
async fn befriend(pool: &sqlx::PgPool, a: Uuid, b: Uuid, key: &SignedSecretKey) {
    let link = link(pool, a, None).await;
    let _ = ok(redeem(pool, &link, b, key).await);
}
fn body(id: Uuid, to: Uuid) -> messages::SendMessage {
    messages::SendMessage {
        id,
        recipient_id: to,
        ciphertext: "opaque ciphertext".into(),
        expires_at: None,
    }
}

#[sqlx::test]
async fn concurrent_claim_one_winner_and_recovery(pool: sqlx::PgPool) {
    let (owner, _) = user(&pool).await;
    let (a, ak) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    let link = link(&pool, owner, None).await;
    let (ra, rb) = tokio::join!(redeem(&pool, &link, a, &ak), redeem(&pool, &link, b, &bk));
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let (winner, key) = if ra.is_ok() { (a, &ak) } else { (b, &bk) };
    let repeated = ok(redeem(&pool, &link, winner, key).await).0;
    assert_eq!(repeated.link.redeemed_by.unwrap().id, winner);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM friendships")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    ok(remove_friend(State(state(pool.clone())), UserId(owner), Path(winner)).await);
    let _ = ok(redeem(&pool, &link, winner, key).await);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM friendships")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "recovery must not restore a removed friendship");
}
#[sqlx::test]
async fn wrong_target_invalid_signature_and_expiry_do_not_consume(pool: sqlx::PgPool) {
    let (owner, _) = user(&pool).await;
    let (a, ak) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    let link = link(&pool, owner, Some(a)).await;
    assert!(redeem(&pool, &link, b, &bk).await.is_err());
    assert!(redeem(&pool, &link, a, &bk).await.is_err());
    assert!(links::preview(
        State(state(pool.clone())),
        UserId(b),
        Json(links::Claim {
            id: link.link.id,
            token: link.token.clone()
        })
    )
    .await
    .is_err());
    let row: Option<Uuid> = sqlx::query_scalar("SELECT redeemed_by FROM friend_links WHERE id=$1")
        .bind(link.link.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(row.is_none());
    let _ = ok(redeem(&pool, &link, a, &ak).await);
    let expired = super::tests::link(&pool, owner, None).await;
    sqlx::query("UPDATE friend_links SET expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(expired.link.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(redeem(&pool, &expired, b, &bk).await.is_err());
    assert!(redeem(&pool, &expired, owner, &bk).await.is_err());
}
#[sqlx::test]
async fn messages_authorization_idempotency_deletion_and_removal(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    let (c, _) = user(&pool).await;
    let id = Uuid::new_v4();
    assert!(
        messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b)))
            .await
            .is_err()
    );
    befriend(&pool, a, b, &bk).await;
    let (r1, r2) = tokio::join!(
        messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))),
        messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b)))
    );
    assert!(r1.is_ok() && r2.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM secret_messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        messages::get(State(state(pool.clone())), UserId(c), Path(id))
            .await
            .is_err()
    );
    assert!(
        messages::delete(State(state(pool.clone())), UserId(c), Path(id))
            .await
            .is_err()
    );
    let listed = ok(messages::list(
        State(state(pool.clone())),
        UserId(b),
        Query(Page::default()),
    )
    .await)
    .0;
    assert_eq!(listed.len(), 1);
    assert!(listed[0].ciphertext.is_none());
    ok(messages::delete(State(state(pool.clone())), UserId(a), Path(id)).await);
    assert!(
        messages::get(State(state(pool.clone())), UserId(a), Path(id))
            .await
            .is_err()
    );
    assert!(
        messages::get(State(state(pool.clone())), UserId(b), Path(id))
            .await
            .is_ok()
    );
    ok(remove_friend(State(state(pool.clone())), UserId(a), Path(b)).await);
    assert!(messages::send(
        State(state(pool.clone())),
        UserId(a),
        Json(body(Uuid::new_v4(), b))
    )
    .await
    .is_err());
    ok(messages::delete(State(state(pool.clone())), UserId(b), Path(id)).await);
    let _ = ok(messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))).await);
    let ciphertext: Option<String> =
        sqlx::query_scalar("SELECT ciphertext FROM secret_messages WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ciphertext.is_none());
    let mut different = body(id, b);
    different.ciphertext = "changed".into();
    assert!(
        messages::send(State(state(pool.clone())), UserId(a), Json(different))
            .await
            .is_err()
    );
}
#[sqlx::test]
async fn expiry_quota_and_failed_claim_rate(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    befriend(&pool, a, b, &bk).await;
    let id = Uuid::new_v4();
    let _ = ok(messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))).await);
    sqlx::query("UPDATE secret_messages SET expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        messages::get(State(state(pool.clone())), UserId(a), Path(id))
            .await
            .is_err()
    );
    assert!(
        messages::get(State(state(pool.clone())), UserId(b), Path(id))
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO social_daily_usage(user_id,kind,count) VALUES($1,'message-send',999) ON CONFLICT(user_id,day,kind) DO UPDATE SET count=999").bind(a).execute(&pool).await.unwrap();
    let (ra, rb) = tokio::join!(
        messages::send(
            State(state(pool.clone())),
            UserId(a),
            Json(body(Uuid::new_v4(), b))
        ),
        messages::send(
            State(state(pool.clone())),
            UserId(a),
            Json(body(Uuid::new_v4(), b))
        )
    );
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    sqlx::query("INSERT INTO social_daily_usage(user_id,kind,count) VALUES($1,'link-claim',999) ON CONFLICT(user_id,day,kind) DO UPDATE SET count=999").bind(a).execute(&pool).await.unwrap();
    for _ in 0..2 {
        assert!(links::preview(
            State(state(pool.clone())),
            UserId(a),
            Json(links::Claim {
                id: Uuid::new_v4(),
                token: "bad".into()
            })
        )
        .await
        .is_err());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count FROM social_daily_usage WHERE user_id=$1 AND kind='link-claim'",
    )
    .bind(a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1000);
}

#[sqlx::test]
async fn revoked_links_token_secrecy_and_pagination(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    let created = link(&pool, a, None).await;
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM friend_links WHERE id=$1")
        .bind(created.link.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(stored, created.token);
    assert_eq!(stored, hash(&created.token));
    assert!(links::preview(
        State(state(pool.clone())),
        UserId(b),
        Json(links::Claim {
            id: created.link.id,
            token: "0".repeat(64)
        })
    )
    .await
    .is_err());
    assert!(
        links::revoke(State(state(pool.clone())), UserId(b), Path(created.link.id))
            .await
            .is_err()
    );
    ok(links::revoke(State(state(pool.clone())), UserId(a), Path(created.link.id)).await);
    assert!(redeem(&pool, &created, b, &bk).await.is_err());
    let _ = link(&pool, a, None).await;
    let first = ok(links::list(
        State(state(pool.clone())),
        UserId(a),
        Query(Page {
            before: None,
            limit: Some(1),
        }),
    )
    .await)
    .0;
    let second = ok(links::list(
        State(state(pool.clone())),
        UserId(a),
        Query(Page {
            before: Some(first[0].id),
            limit: Some(1),
        }),
    )
    .await)
    .0;
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
    let other = ok(links::list(
        State(state(pool.clone())),
        UserId(b),
        Query(Page::default()),
    )
    .await)
    .0;
    assert!(other.is_empty());
}

#[sqlx::test]
async fn mailbox_capacity_serializes_and_precise_expiry_retries(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    befriend(&pool, a, b, &bk).await;
    let id = Uuid::new_v4();
    let expiry = Utc::now() + chrono::Duration::hours(1);
    for _ in 0..2 {
        let mut send_body = body(id, b);
        send_body.expires_at = Some(expiry);
        let _ = ok(messages::send(State(state(pool.clone())), UserId(a), Json(send_body)).await);
    }
    // Fill all but one slot using fixtures, then contend for the final slot.
    sqlx::query("INSERT INTO secret_messages(id,sender_id,recipient_id,ciphertext,ciphertext_hash,sender_public_key,recipient_public_key) SELECT gen_random_uuid(),$1,$2,'fixture','hash','key','key' FROM generate_series(1,998)")
        .bind(a).bind(b).execute(&pool).await.unwrap();
    let (ra, rb) = tokio::join!(
        messages::send(
            State(state(pool.clone())),
            UserId(a),
            Json(body(Uuid::new_v4(), b))
        ),
        messages::send(
            State(state(pool.clone())),
            UserId(a),
            Json(body(Uuid::new_v4(), b))
        )
    );
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM secret_messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1000);
}

#[sqlx::test]
async fn histories_are_chronological_with_owned_cursors_and_ties(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    let (outsider, _) = user(&pool).await;
    befriend(&pool, a, b, &bk).await;
    let ids = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
    for id in ids {
        let _ = ok(messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))).await);
    }
    sqlx::query("UPDATE secret_messages SET created_at='2026-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE secret_messages SET created_at='2026-01-02T00:00:00Z' WHERE id=$1")
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();
    let mut cursor = None;
    for expected in [ids[0], ids[2], ids[1]] {
        let page = ok(messages::list(
            State(state(pool.clone())),
            UserId(b),
            Query(Page {
                before: cursor,
                limit: Some(1),
            }),
        )
        .await)
        .0;
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, expected);
        cursor = Some(expected);
    }
    assert!(messages::list(
        State(state(pool.clone())),
        UserId(outsider),
        Query(Page {
            before: Some(ids[0]),
            limit: Some(1)
        })
    )
    .await
    .is_err());
    let mut link_ids = Vec::new();
    for _ in 0..3 {
        link_ids.push(link(&pool, a, None).await.link.id);
    }
    link_ids.sort();
    sqlx::query(
        "UPDATE friend_links SET created_at='2026-01-01T00:00:00Z' WHERE redeemed_by IS NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE friend_links SET created_at='2026-01-02T00:00:00Z' WHERE id=$1")
        .bind(link_ids[0])
        .execute(&pool)
        .await
        .unwrap();
    // Exclude the befriend fixture above from the first page.
    sqlx::query(
        "UPDATE friend_links SET created_at='2025-01-01T00:00:00Z' WHERE redeemed_by IS NOT NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut cursor = None;
    for expected in [link_ids[0], link_ids[2], link_ids[1]] {
        let page = ok(links::list(
            State(state(pool.clone())),
            UserId(a),
            Query(Page {
                before: cursor,
                limit: Some(1),
            }),
        )
        .await)
        .0;
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, expected);
        cursor = Some(expected);
    }
    assert!(links::list(
        State(state(pool.clone())),
        UserId(b),
        Query(Page {
            before: Some(link_ids[0]),
            limit: Some(1)
        })
    )
    .await
    .is_err());
}

#[sqlx::test]
async fn key_snapshots_are_canonical_accounted_and_erased(pool: sqlx::PgPool) {
    let (a, _) = user(&pool).await;
    let (b, bk) = user(&pool).await;
    befriend(&pool, a, b, &bk).await;
    let original: String = sqlx::query_scalar("SELECT public_key FROM users WHERE id=$1")
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();
    let padded = original.replacen(
        "-----BEGIN PGP PUBLIC KEY BLOCK-----\n",
        &format!(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\nComment: {}\n",
            "X".repeat(64 * 1024)
        ),
        1,
    );
    assert!(padded.len() > original.len() + 60 * 1024);
    sqlx::query("UPDATE users SET public_key=$2 WHERE id=$1")
        .bind(a)
        .bind(padded)
        .execute(&pool)
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let sent = ok(messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))).await).0;
    assert!(sent.sender_public_key.is_empty());
    assert!(sent.recipient_public_key.is_empty());
    let got = ok(messages::get(State(state(pool.clone())), UserId(b), Path(id)).await).0;
    assert_eq!(got.sender_public_key, original);
    let listed = ok(messages::list(
        State(state(pool.clone())),
        UserId(b),
        Query(Page::default()),
    )
    .await)
    .0;
    assert!(listed[0].sender_public_key.is_empty());
    assert!(listed[0].recipient_public_key.is_empty());
    // Existing snapshot bytes count even though ciphertext itself is tiny.
    sqlx::query("UPDATE secret_messages SET sender_public_key=repeat('k',20971520) WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(messages::send(
        State(state(pool.clone())),
        UserId(a),
        Json(body(Uuid::new_v4(), b))
    )
    .await
    .is_err());
    ok(messages::delete(State(state(pool.clone())), UserId(a), Path(id)).await);
    ok(messages::delete(State(state(pool.clone())), UserId(b), Path(id)).await);
    let (ciphertext, sender, recipient): (Option<String>, String, String) = sqlx::query_as(
        "SELECT ciphertext,sender_public_key,recipient_public_key FROM secret_messages WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(ciphertext.is_none());
    assert!(sender.is_empty());
    assert!(recipient.is_empty());
    let _ = ok(messages::send(State(state(pool.clone())), UserId(a), Json(body(id, b))).await);
    let expired = Uuid::new_v4();
    let _ = ok(messages::send(
        State(state(pool.clone())),
        UserId(a),
        Json(body(expired, b)),
    )
    .await);
    sqlx::query("UPDATE secret_messages SET expires_at=now()-interval '1 hour' WHERE id=$1")
        .bind(expired)
        .execute(&pool)
        .await
        .unwrap();
    let _ = ok(messages::send(
        State(state(pool.clone())),
        UserId(a),
        Json(body(Uuid::new_v4(), b)),
    )
    .await);
    let (ciphertext, sender, recipient): (Option<String>, String, String) = sqlx::query_as(
        "SELECT ciphertext,sender_public_key,recipient_public_key FROM secret_messages WHERE id=$1",
    )
    .bind(expired)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(ciphertext.is_none());
    assert!(sender.is_empty());
    assert!(recipient.is_empty());
}
