use super::*;
use uuid::Uuid;

#[utoipa::path(
    delete, path = "/{project_id}/delete", tag = PROJECT_TAG,
    security(("bearer" = [])),
    responses((status = 200, description = "Project deleted"),
        (status = 403, description = "Not a project member"),
        (status = 409, description = "Shared projects cannot be deleted"))
)]
pub async fn delete_project(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(project_id): Path<Uuid>,
) -> Result<(), AppError> {
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT id FROM projects WHERE id = $1 FOR UPDATE")
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
    let members: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM user_project_relations WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;
    if !members.contains(&user_id) {
        return Err((axum::http::StatusCode::FORBIDDEN, "Not a project member").into());
    }
    if members.len() != 1 {
        return Err((
            axum::http::StatusCode::CONFLICT,
            "Shared projects cannot be deleted; you must be the sole remaining member",
        )
            .into());
    }
    sqlx::query("DELETE FROM variables WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    // Memberships and outstanding invitations cascade with the project.
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[sqlx::test]
    async fn deletes_only_sole_member_project(pool: sqlx::PgPool) {
        let user: Uuid = sqlx::query_scalar(
            "INSERT INTO users(username, public_key) VALUES('a','a') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let other: Uuid = sqlx::query_scalar(
            "INSERT INTO users(username, public_key) VALUES('b','b') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let project: Uuid = sqlx::query_scalar("INSERT INTO projects DEFAULT VALUES RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_project_relations(user_id,project_id) VALUES($1,$2),($3,$2)")
            .bind(user)
            .bind(project)
            .bind(other)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO variables(value,project_id) VALUES('ciphertext',$1)")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        let state = AppState {
            db: Arc::new(pool.clone()),
            caps: Arc::new(crate::config::Caps::from_env()),
        };
        assert!(
            delete_project(State(state.clone()), UserId(Uuid::new_v4()), Path(project))
                .await
                .is_err()
        );
        assert!(
            delete_project(State(state.clone()), UserId(user), Path(project))
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM user_project_relations WHERE user_id=$1")
            .bind(other)
            .execute(&pool)
            .await
            .unwrap();
        assert!(delete_project(State(state), UserId(user), Path(project))
            .await
            .is_ok());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM variables WHERE project_id=$1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
