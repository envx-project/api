use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::{extractors::user::UserId, structs::ProjectInvite};

use super::*;

#[derive(Deserialize, ToSchema)]
pub struct AcceptInviteBody {
    pub code: Uuid,
    pub verifier: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct AcceptInviteReturnType {
    pub project_id: String,
    pub invite_id: String,
    pub ciphertext: String,
}

#[utoipa::path(
    post,
    path = "/accept",
    tag = INVITE_TAG,
    responses(
        (status = 200, description = "Success", body = AcceptInviteReturnType),
        (status = 400, description = "Invalid public key"),
        (status = 404, description = "Invite not found"),
        (status = 409, description = "Invite already accepted"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn accept_invite(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(body): Json<AcceptInviteBody>,
) -> Result<Json<AcceptInviteReturnType>, AppError> {
    let invite = sqlx::query_as!(
        ProjectInvite,
        "SELECT * FROM project_invites WHERE id = $1",
        body.code
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to fetch invite")?;

    let parsed_hash = PasswordHash::new(&invite.verifier_argon2id)
        .map_err(|_| AppError::Error(Errors::Unauthorized))?;
    let is_valid = Argon2::default()
        .verify_password(body.verifier.as_bytes(), &parsed_hash)
        .is_ok();

    if !is_valid {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    if invite.invited_id.is_some() {
        return Err(AppError::Generic(
            StatusCode::CONFLICT,
            "Invite already accepted".into(),
        ));
    }

    let mut tx = state.db.begin().await?;
    // An invitation cannot outlive its author's authority to grant access.
    let author_membership: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM user_project_relations WHERE user_id=$1 AND project_id=$2 FOR SHARE",
    )
    .bind(invite.author_id)
    .bind(invite.project_id)
    .fetch_optional(&mut *tx)
    .await?;
    if author_membership.is_none() {
        return Err(AppError::Error(Errors::Unauthorized));
    }
    let id = sqlx::query!(
        "UPDATE project_invites 
        SET invited_id = $1,
        ciphertext = NULL 
        WHERE id = $2
            AND invited_id IS NULL
            AND ciphertext IS NOT NULL
            AND expires_at > NOW()
        RETURNING id;
        ",
        user_id,
        body.code
    )
    .fetch_optional(&mut *tx)
    .await
    .context("Failed to update invite")?;

    if id.is_none() {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING",
        &user_id,
        &invite.project_id,
    )
    .execute(&mut *tx)
    .await
    .context("Failed to insert user project relations")?;

    tx.commit().await?;

    Ok(Json(AcceptInviteReturnType {
        ciphertext: invite.ciphertext.unwrap(),
        invite_id: invite.id.to_string(),
        project_id: invite.project_id.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        PasswordHasher,
    };
    #[sqlx::test]
    async fn expired_invite_does_not_grant_membership(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let guest = user(&pool).await;
        let project = project(&pool, owner).await;
        let verifier = Uuid::new_v4();
        let hash = Argon2::default()
            .hash_password(verifier.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        let code=sqlx::query_scalar("INSERT INTO project_invites(project_id,author_id,expires_at,verifier_argon2id,ciphertext) VALUES ($1,$2,now()-interval '1 hour',$3,'secret') RETURNING id").bind(project).bind(owner).bind(hash).fetch_one(&pool).await.unwrap();
        assert!(accept_invite(
            State(state(pool.clone())),
            UserId(guest),
            Json(AcceptInviteBody { code, verifier })
        )
        .await
        .is_err());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM user_project_relations WHERE user_id=$1 AND project_id=$2",
        )
        .bind(guest)
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }
    #[sqlx::test]
    async fn removed_author_cannot_grant_membership(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let guest = user(&pool).await;
        let project = project(&pool, owner).await;
        let verifier = Uuid::new_v4();
        let hash = Argon2::default()
            .hash_password(verifier.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        let code=sqlx::query_scalar("INSERT INTO project_invites(project_id,author_id,expires_at,verifier_argon2id,ciphertext) VALUES ($1,$2,now()+interval '1 hour',$3,'secret') RETURNING id").bind(project).bind(owner).bind(hash).fetch_one(&pool).await.unwrap();
        sqlx::query("DELETE FROM user_project_relations WHERE user_id=$1 AND project_id=$2")
            .bind(owner)
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        assert!(accept_invite(
            State(state(pool.clone())),
            UserId(guest),
            Json(AcceptInviteBody { code, verifier })
        )
        .await
        .is_err());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM user_project_relations WHERE user_id=$1 AND project_id=$2",
        )
        .bind(guest)
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }
    #[sqlx::test]
    async fn concurrent_redemption_has_one_winner(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let a = user(&pool).await;
        let b = user(&pool).await;
        let project = project(&pool, owner).await;
        let verifier = Uuid::new_v4();
        let hash = Argon2::default()
            .hash_password(verifier.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        let code=sqlx::query_scalar("INSERT INTO project_invites(project_id,author_id,expires_at,verifier_argon2id,ciphertext) VALUES ($1,$2,now()+interval '1 hour',$3,'secret') RETURNING id").bind(project).bind(owner).bind(hash).fetch_one(&pool).await.unwrap();
        let (ra, rb) = tokio::join!(
            accept_invite(
                State(state(pool.clone())),
                UserId(a),
                Json(AcceptInviteBody { code, verifier })
            ),
            accept_invite(
                State(state(pool.clone())),
                UserId(b),
                Json(AcceptInviteBody { code, verifier })
            )
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
}
