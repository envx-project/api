use super::*;
use crate::test_support::{state, user};

fn body(action: &str, token: Option<&str>, message: Option<&str>) -> Exchange {
    Exchange {
        action: action.into(),
        token: token.map(str::to_owned),
        message: message.map(str::to_owned),
    }
}
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
#[sqlx::test]
async fn pairing_is_bound_to_owner_receiver_and_phase(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let stranger = user(&pool).await;
    let state = state(pool.clone());
    let id = create(
        State(state.clone()),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap()),
    )
    .await
    .ok()
    .unwrap()
    .0
    .id;
    assert!(source(
        State(state.clone()),
        UserId(stranger),
        Path(id),
        Json(body("poll", None, None))
    )
    .await
    .is_err());
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("claim", Some(TOKEN), Some("00")))
    )
    .await
    .is_ok());
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("claim", Some(TOKEN), Some("00")))
    )
    .await
    .is_err());
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("poll", Some(&"f".repeat(64)), None))
    )
    .await
    .is_err());
    assert!(source(
        State(state.clone()),
        UserId(owner),
        Path(id),
        Json(body("payload", None, Some("00")))
    )
    .await
    .is_err());
    assert_eq!(
        source(
            State(state.clone()),
            UserId(owner),
            Path(id),
            Json(body("poll", None, None))
        )
        .await
        .ok()
        .unwrap()
        .0
        .message
        .as_deref(),
        Some("00")
    );
    let _ = source(
        State(state.clone()),
        UserId(owner),
        Path(id),
        Json(body("handshake", None, Some("01"))),
    )
    .await
    .ok()
    .unwrap();
    assert_eq!(
        receiver(
            State(state.clone()),
            Path(id),
            Json(body("poll", Some(TOKEN), None))
        )
        .await
        .ok()
        .unwrap()
        .0
        .message
        .as_deref(),
        Some("01")
    );
    let _ = receiver(
        State(state.clone()),
        Path(id),
        Json(body("handshake", Some(TOKEN), Some("02"))),
    )
    .await
    .ok()
    .unwrap();
    let _ = source(
        State(state.clone()),
        UserId(owner),
        Path(id),
        Json(body("payload", None, Some("03"))),
    )
    .await
    .ok()
    .unwrap();
    assert_eq!(
        receiver(
            State(state.clone()),
            Path(id),
            Json(body("poll", Some(TOKEN), None))
        )
        .await
        .ok()
        .unwrap()
        .0
        .message
        .as_deref(),
        Some("03")
    );
    let _ = receiver(
        State(state.clone()),
        Path(id),
        Json(body("ack", Some(TOKEN), None)),
    )
    .await
    .ok()
    .unwrap();
    assert!(receiver(
        State(state),
        Path(id),
        Json(body("poll", Some(TOKEN), None))
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_pairings")
        .fetch_one(&pool)
        .await
        .ok()
        .unwrap();
    assert_eq!(count, 0);
}
#[sqlx::test]
async fn pairing_expires_cancels_and_bounds_storage(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let state = state(pool.clone());
    let id = create(
        State(state.clone()),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap()),
    )
    .await
    .ok()
    .unwrap()
    .0
    .id;
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("claim", Some(TOKEN), Some(&"00".repeat(1025))))
    )
    .await
    .is_err());
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("claim", Some("short"), Some("00")))
    )
    .await
    .is_err());
    sqlx::query("UPDATE auth_pairings SET expires_at=now()-interval '1 second'")
        .execute(&pool)
        .await
        .ok()
        .unwrap();
    assert!(receiver(
        State(state.clone()),
        Path(id),
        Json(body("claim", Some(TOKEN), Some("00")))
    )
    .await
    .is_err());
    cleanup(&pool).await.ok().unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_pairings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let id = create(
        State(state.clone()),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap()),
    )
    .await
    .ok()
    .unwrap()
    .0
    .id;
    let _ = source(
        State(state.clone()),
        UserId(owner),
        Path(id),
        Json(body("cancel", None, None)),
    )
    .await
    .ok()
    .unwrap();
    assert!(source(
        State(state.clone()),
        UserId(owner),
        Path(id),
        Json(body("poll", None, None))
    )
    .await
    .is_err());
    for _ in 0..3 {
        let _ = create(
            State(state.clone()),
            UserId(owner),
            ClientIp("192.0.2.1".parse().unwrap()),
        )
        .await
        .ok()
        .unwrap();
    }
    assert!(create(
        State(state),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap())
    )
    .await
    .is_err());
}

#[sqlx::test]
async fn creating_accounts_cannot_bypass_pairing_client_quota(pool: sqlx::PgPool) {
    let state = state(pool.clone());
    for _ in 0..12 {
        let owner = user(&pool).await;
        let _ = create(
            State(state.clone()),
            UserId(owner),
            ClientIp("192.0.2.1".parse().unwrap()),
        )
        .await
        .ok()
        .unwrap();
    }
    let owner = user(&pool).await;
    assert!(create(
        State(state),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap())
    )
    .await
    .is_err());
}

#[sqlx::test]
async fn only_one_concurrent_receiver_claims_and_capability_is_hashed(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let state = state(pool.clone());
    let id = create(
        State(state.clone()),
        UserId(owner),
        ClientIp("192.0.2.1".parse().unwrap()),
    )
    .await
    .ok()
    .unwrap()
    .0
    .id;
    let other = "a".repeat(64);
    let (a, b) = tokio::join!(
        receiver(
            State(state.clone()),
            Path(id),
            Json(body("claim", Some(TOKEN), Some("00")))
        ),
        receiver(
            State(state.clone()),
            Path(id),
            Json(body("claim", Some(&other), Some("01")))
        )
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let stored: String = sqlx::query_scalar("SELECT receiver_hash FROM auth_pairings WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(stored, TOKEN);
    assert_ne!(stored, other);
    assert_eq!(stored, hash(if a.is_ok() { TOKEN } else { &other }));
}
