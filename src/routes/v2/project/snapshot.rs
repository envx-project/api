use super::*;
use crate::{
    extractors::client_ip::ClientIp,
    helpers::project_snapshot::{self as snapshots, RewrappedVariable, Snapshot},
};
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct SnapshotBody {
    #[serde(default)]
    pub add_user_ids: Vec<Uuid>,
}
#[utoipa::path(post, path="/{project_id}/snapshot", tag=PROJECT_TAG, responses((status=200,body=Snapshot)), security(("bearer"=[])))]
pub async fn snapshot(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(project): Path<Uuid>,
    Json(body): Json<SnapshotBody>,
) -> Result<Json<Snapshot>, AppError> {
    let mut tx = state.db.begin().await?;
    snapshots::lock_project(&mut tx, project).await?;
    snapshots::authorize(&mut tx, project, user).await?;
    let result = snapshots::read(&mut tx, project, &body.add_user_ids).await?;
    tx.commit().await?;
    Ok(Json(result))
}
#[derive(Deserialize, ToSchema)]
pub struct RewrapBody {
    pub protocol_version: Option<u8>,
    pub snapshot: String,
    pub variables: Vec<RewrappedVariable>,
    pub add_user_ids: Vec<Uuid>,
}
#[utoipa::path(post, path="/{project_id}/rewrap", tag=PROJECT_TAG, responses((status=200),(status=409,description="Snapshot changed")), security(("bearer"=[])))]
pub async fn rewrap(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(project): Path<Uuid>,
    ClientIp(ip): ClientIp,
    Json(body): Json<RewrapBody>,
) -> Result<(), AppError> {
    snapshots::require_version(body.protocol_version)?;
    let mut tx = state.db.begin().await?;
    snapshots::lock_project(&mut tx, project).await?;
    snapshots::authorize(&mut tx, project, user).await?;
    let current = snapshots::read(&mut tx, project, &body.add_user_ids).await?;
    snapshots::rewrap(
        &state,
        &mut tx,
        &current,
        &body.snapshot,
        &body.variables,
        user,
        ip,
    )
    .await?;
    sqlx::query("INSERT INTO user_project_relations(user_id,project_id) SELECT user_id,$1 FROM UNNEST($2::uuid[]) t(user_id) ON CONFLICT DO NOTHING")
        .bind(project).bind(body.add_user_ids).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    fn ip() -> ClientIp {
        ClientIp("127.0.0.1".parse().unwrap())
    }
    #[sqlx::test]
    async fn adding_users_rejects_stale_or_partial_batches_and_joins_atomically(
        pool: sqlx::PgPool,
    ) {
        let owner = user(&pool).await;
        let guest = user(&pool).await;
        let project = project(&pool, owner).await;
        let id:Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES(gen_random_uuid(),$1,'original') RETURNING id").bind(project).fetch_one(&pool).await.unwrap();
        let read = || {
            snapshot(
                State(state(pool.clone())),
                UserId(owner),
                Path(project),
                Json(SnapshotBody {
                    add_user_ids: vec![guest],
                }),
            )
        };
        let old = read().await.ok().unwrap().0;
        sqlx::query("UPDATE variables SET value='changed' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let apply = |snapshot: String, variables: Vec<RewrappedVariable>| {
            rewrap(
                State(state(pool.clone())),
                UserId(owner),
                Path(project),
                ip(),
                Json(RewrapBody {
                    protocol_version: Some(1),
                    snapshot,
                    variables,
                    add_user_ids: vec![guest],
                }),
            )
        };
        assert!(matches!(
            apply(
                old.snapshot,
                vec![RewrappedVariable {
                    id,
                    value: "rewrapped".into()
                }]
            )
            .await,
            Err(AppError::Protocol(_, "project_snapshot_stale", _))
        ));
        let fresh = read().await.ok().unwrap().0;
        assert!(apply(fresh.snapshot.clone(), vec![]).await.is_err());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM user_project_relations WHERE project_id=$1 AND user_id=$2",
        )
        .bind(project)
        .bind(guest)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        assert!(apply(
            fresh.snapshot,
            vec![RewrappedVariable {
                id,
                value: "rewrapped".into()
            }]
        )
        .await
        .is_ok());
        let value:String=sqlx::query_scalar("SELECT value FROM variables v JOIN user_project_relations r ON r.project_id=v.project_id WHERE v.id=$1 AND r.user_id=$2").bind(id).bind(guest).fetch_one(&pool).await.unwrap();
        assert_eq!(value, "rewrapped");
    }
}
