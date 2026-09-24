use super::*;
use crate::{
    routes::v2::invite::new::{new_invite, InviteBody},
    test_support::*,
};
fn ip() -> ClientIp {
    ClientIp("127.0.0.1".parse().unwrap())
}
async fn issue(pool: &sqlx::PgPool, owner: Uuid, guest: Uuid, project: Uuid) -> AcceptInviteBody {
    let mut tx = pool.begin().await.unwrap();
    snapshots::lock_project(&mut tx, project)
        .await
        .ok()
        .unwrap();
    let snapshot = snapshots::read(&mut tx, project, &[]).await.ok().unwrap();
    tx.commit().await.unwrap();
    let invite = new_invite(
        State(state(pool.clone())),
        UserId(owner),
        Json(InviteBody {
            project_id: project,
            ciphertext: "encrypted-invite".into(),
            protocol_version: Some(1),
            snapshot: Some(snapshot.snapshot),
        }),
    )
    .await
    .ok()
    .unwrap()
    .0;
    let prepared = prepare_invite(
        State(state(pool.clone())),
        UserId(guest),
        Json(PrepareInviteBody {
            code: invite.invite_code,
            verifier: invite.verifier,
        }),
    )
    .await
    .ok()
    .unwrap()
    .0;
    AcceptInviteBody {
        code: invite.invite_code,
        verifier: invite.verifier,
        protocol_version: Some(1),
        snapshot: Some(prepared.snapshot),
        variables: Some(
            snapshot
                .variables
                .into_iter()
                .map(|v| RewrappedVariable {
                    id: v.id,
                    value: "rewrapped".into(),
                })
                .collect(),
        ),
    }
}
async fn assert_unclaimed(pool: &sqlx::PgPool, code: Uuid, guest: Uuid, project: Uuid) {
    let (claimed, ciphertext): (Option<Uuid>, Option<String>) =
        sqlx::query_as("SELECT invited_id,ciphertext FROM project_invites WHERE id=$1")
            .bind(code)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(claimed.is_none());
    assert!(ciphertext.is_some());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_project_relations WHERE project_id=$1 AND user_id=$2",
    )
    .bind(project)
    .bind(guest)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}
async fn variable(pool: &sqlx::PgPool, project: Uuid) -> Uuid {
    sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES(gen_random_uuid(),$1,'original') RETURNING id").bind(project).fetch_one(pool).await.unwrap()
}
#[sqlx::test]
async fn legacy_invite_cannot_join_before_variables_are_rewrapped(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    let body = issue(&pool, owner, guest, project).await;
    sqlx::query("UPDATE project_invites SET snapshot_hash=NULL WHERE id=$1")
        .bind(body.code)
        .execute(&pool)
        .await
        .unwrap();
    assert!(prepare_invite(
        State(state(pool.clone())),
        UserId(guest),
        Json(PrepareInviteBody {
            code: body.code,
            verifier: body.verifier
        })
    )
    .await
    .is_err());
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(body.clone())
    )
    .await
    .is_err());
    let legacy = AcceptInviteBody {
        protocol_version: None,
        snapshot: None,
        variables: None,
        ..body.clone()
    };
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(legacy)
    )
    .await
    .is_err());
    assert_unclaimed(&pool, body.code, guest, project).await;
}
#[sqlx::test]
async fn expired_invite_does_not_grant_membership(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    let body = issue(&pool, owner, guest, project).await;
    sqlx::query("UPDATE project_invites SET expires_at=now()-interval '1 hour' WHERE id=$1")
        .bind(body.code)
        .execute(&pool)
        .await
        .unwrap();
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(body.clone())
    )
    .await
    .is_err());
    assert_unclaimed(&pool, body.code, guest, project).await;
}
#[sqlx::test]
async fn removed_author_cannot_grant_membership(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    let body = issue(&pool, owner, guest, project).await;
    snapshots::remove_members(&state(pool.clone()), project, owner, &[owner])
        .await
        .ok()
        .unwrap();
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(body.clone())
    )
    .await
    .is_err());
    assert_unclaimed(&pool, body.code, guest, project).await;
}
#[sqlx::test]
async fn changed_added_deleted_or_replaced_variables_and_membership_reject_stale_invites(
    pool: sqlx::PgPool,
) {
    for change in ["changed", "added", "deleted", "replaced", "membership"] {
        let owner = user(&pool).await;
        let guest = user(&pool).await;
        let project = project(&pool, owner).await;
        let id = variable(&pool, project).await;
        let body = issue(&pool, owner, guest, project).await;
        match change {
            "changed" => {
                sqlx::query("UPDATE variables SET value='changed' WHERE id=$1")
                    .bind(id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "added" => {
                variable(&pool, project).await;
            }
            "deleted" => {
                sqlx::query("DELETE FROM variables WHERE id=$1")
                    .bind(id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "replaced" => {
                crate::helpers::variables::replace_many(
                    &state(pool.clone()),
                    owner,
                    ip().0,
                    project,
                    vec!["original".into()],
                    vec![id],
                )
                .await
                .ok()
                .unwrap();
            }
            _ => {
                let extra = user(&pool).await;
                sqlx::query("INSERT INTO user_project_relations(project_id,user_id) VALUES($1,$2)")
                    .bind(project)
                    .bind(extra)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        let before: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT id,value FROM variables WHERE project_id=$1 ORDER BY id")
                .bind(project)
                .fetch_all(&pool)
                .await
                .unwrap();
        let result = accept_invite(
            State(state(pool.clone())),
            UserId(guest),
            ip(),
            Json(body.clone()),
        )
        .await;
        assert!(
            matches!(
                result,
                Err(AppError::Protocol(_, "project_snapshot_stale", _))
            ),
            "{change}"
        );
        assert_unclaimed(&pool, body.code, guest, project).await;
        let after: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT id,value FROM variables WHERE project_id=$1 ORDER BY id")
                .bind(project)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(before, after, "{change}");
    }
}
#[sqlx::test]
async fn failed_or_incomplete_rewrap_does_not_join_or_consume(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    variable(&pool, project).await;
    variable(&pool, project).await;
    let body = issue(&pool, owner, guest, project).await;
    let mut incomplete = body.clone();
    incomplete.variables.as_mut().unwrap().pop();
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(incomplete)
    )
    .await
    .is_err());
    let mut duplicate = body.clone();
    let first = duplicate.variables.as_ref().unwrap()[0].clone();
    duplicate.variables.as_mut().unwrap().push(first);
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(duplicate)
    )
    .await
    .is_err());
    sqlx::query("ALTER TABLE variables ADD CONSTRAINT reject_rewrap CHECK(value<>'reject-me')")
        .execute(&pool)
        .await
        .unwrap();
    let mut failed = body.clone();
    failed.variables.as_mut().unwrap()[1].value = "reject-me".into();
    assert!(accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(failed)
    )
    .await
    .is_err());
    assert_unclaimed(&pool, body.code, guest, project).await;
    let originals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM variables WHERE project_id=$1 AND value='original'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(originals, 2);
    assert!(
        accept_invite(State(state(pool.clone())), UserId(guest), ip(), Json(body))
            .await
            .is_ok()
    );
}
#[sqlx::test]
async fn concurrent_redemption_has_one_winner_and_empty_project_works(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let a = user(&pool).await;
    let b = user(&pool).await;
    let project = project(&pool, owner).await;
    let body = issue(&pool, owner, a, project).await;
    let prepared = prepare_invite(
        State(state(pool.clone())),
        UserId(b),
        Json(PrepareInviteBody {
            code: body.code,
            verifier: body.verifier,
        }),
    )
    .await
    .ok()
    .unwrap()
    .0;
    let other = AcceptInviteBody {
        snapshot: Some(prepared.snapshot),
        ..body.clone()
    };
    let (ra, rb) = tokio::join!(
        accept_invite(State(state(pool.clone())), UserId(a), ip(), Json(body)),
        accept_invite(State(state(pool.clone())), UserId(b), ip(), Json(other))
    );
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_project_relations WHERE project_id=$1 AND user_id<>$2",
    )
    .bind(project)
    .bind(owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn creator_must_submit_the_fetched_snapshot_and_payload_quotas_are_bounded(
    pool: sqlx::PgPool,
) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    let body = issue(&pool, owner, guest, project).await;
    let source: String =
        sqlx::query_scalar("SELECT snapshot_hash FROM project_invites WHERE id=$1")
            .bind(body.code)
            .fetch_one(&pool)
            .await
            .unwrap();
    let create = |ciphertext: String, snapshot: String| {
        new_invite(
            State(state(pool.clone())),
            UserId(owner),
            Json(InviteBody {
                project_id: project,
                ciphertext,
                protocol_version: Some(1),
                snapshot: Some(snapshot),
            }),
        )
    };
    assert!(create("x".repeat(1024 * 1024 + 1), source.clone())
        .await
        .is_err());
    sqlx::query("UPDATE project_invites SET expires_at=now()-interval '1 hour' WHERE id=$1")
        .bind(body.code)
        .execute(&pool)
        .await
        .unwrap();
    assert!(create("valid".into(), source.clone()).await.is_ok());
    let expired: Option<String> =
        sqlx::query_scalar("SELECT ciphertext FROM project_invites WHERE id=$1")
            .bind(body.code)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(expired.is_none());
    sqlx::query("INSERT INTO project_invites(project_id,author_id,expires_at,verifier_argon2id,ciphertext,snapshot_hash) SELECT $1,$2,now()+interval '1 hour','fixture','payload',$3 FROM generate_series(1,31)").bind(project).bind(owner).bind(&source).execute(&pool).await.unwrap();
    assert!(create("valid".into(), source.clone()).await.is_err());
    // Count and byte limits are independent.
    sqlx::query("UPDATE project_invites SET ciphertext=repeat('x',16777216) WHERE id=(SELECT id FROM project_invites WHERE author_id=$1 AND expires_at>now() LIMIT 1)").bind(owner).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM project_invites WHERE author_id=$1 AND length(ciphertext)<16777216")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    assert!(create("valid".into(), source.clone()).await.is_err());
    variable(&pool, project).await;
    assert!(matches!(
        create("valid".into(), source).await,
        Err(AppError::Protocol(_, "project_snapshot_stale", _))
    ));
}

#[sqlx::test]
async fn writer_holds_project_lock_until_commit_and_acceptance_observes_it(pool: sqlx::PgPool) {
    let owner = user(&pool).await;
    let guest = user(&pool).await;
    let project = project(&pool, owner).await;
    let id = variable(&pool, project).await;
    let body = issue(&pool, owner, guest, project).await;
    let mut tx = pool.begin().await.unwrap();
    snapshots::lock_project(&mut tx, project)
        .await
        .ok()
        .unwrap();
    sqlx::query("UPDATE variables SET value='concurrent-newer-value' WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let accept = accept_invite(
        State(state(pool.clone())),
        UserId(guest),
        ip(),
        Json(body.clone()),
    );
    tokio::pin!(accept);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(75), &mut accept)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    assert!(matches!(
        accept.await,
        Err(AppError::Protocol(_, "project_snapshot_stale", _))
    ));
    assert_unclaimed(&pool, body.code, guest, project).await;
}
