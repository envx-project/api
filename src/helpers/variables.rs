use super::caps;
use crate::{error::Errors, structs::Variable, AppError, AppState};
use axum::http::StatusCode;
use std::{collections::HashSet, net::IpAddr};
use uuid::Uuid;

pub async fn update_many(
    state: &AppState,
    user_id: Uuid,
    ip: IpAddr,
    variables: Vec<Variable>,
) -> Result<Vec<String>, AppError> {
    let mut seen = HashSet::new();
    let mut parsed = Vec::new();
    for variable in &variables {
        let id = Uuid::parse_str(&variable.id).map_err(|_| {
            AppError::Generic(StatusCode::BAD_REQUEST, "Invalid variable ID".into())
        })?;
        let project = Uuid::parse_str(&variable.project_id)
            .map_err(|_| AppError::Generic(StatusCode::BAD_REQUEST, "Invalid project ID".into()))?;
        if !seen.insert(id) {
            return Err(AppError::Generic(
                StatusCode::BAD_REQUEST,
                "Duplicate variable ID".into(),
            ));
        }
        parsed.push((id, project));
    }
    // Stable lock order prevents opposing batches from deadlocking.
    parsed.sort_unstable();
    let mut tx = state.db.begin().await?;
    for (id, project) in &parsed {
        let authorized: Option<Uuid> = sqlx::query_scalar("SELECT v.id FROM variables v JOIN user_project_relations upr ON upr.project_id=v.project_id WHERE v.id=$1 AND v.project_id=$2 AND upr.user_id=$3 FOR UPDATE OF v FOR SHARE OF upr")
            .bind(id).bind(project).bind(user_id).fetch_optional(&mut *tx).await?;
        if authorized.is_none() {
            return Err(Errors::Unauthorized.into());
        }
    }
    caps::check_per_value(
        &state.caps,
        &variables
            .iter()
            .map(|v| v.value.as_str())
            .collect::<Vec<_>>(),
    )?;
    let mut grouped = std::collections::HashMap::<Uuid, (Vec<Uuid>, Vec<&str>)>::new();
    for variable in &variables {
        let group = grouped
            .entry(Uuid::parse_str(&variable.project_id)?)
            .or_default();
        group.0.push(Uuid::parse_str(&variable.id)?);
        group.1.push(&variable.value);
    }
    let mut delta = 0;
    for (project, (ids, values)) in grouped {
        caps::check_project_for_update_on(&state.caps, &mut tx, project, &ids, &values).await?;
        let old: i64 = sqlx::query_scalar("SELECT COALESCE(sum(octet_length(value)),0)::bigint FROM variables WHERE project_id=$1 AND id=ANY($2)")
            .bind(project).bind(ids).fetch_one(&mut *tx).await?;
        delta += values.iter().map(|v| v.len() as i64).sum::<i64>() - old;
    }
    caps::check_user_total_on(&state.caps, &mut tx, user_id, delta).await?;
    caps::check_and_record_ip_on(&state.caps, &mut tx, ip, delta.max(0)).await?;
    let mut ids = Vec::new();
    for variable in variables {
        let id = Uuid::parse_str(&variable.id)?;
        sqlx::query("UPDATE variables SET value=$1 WHERE id=$2")
            .bind(variable.value)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        ids.push(id.to_string());
    }
    tx.commit().await?;
    Ok(ids)
}

pub async fn replace_many(
    state: &AppState,
    user_id: Uuid,
    ip: IpAddr,
    project: Uuid,
    values: Vec<String>,
    replace_ids: Vec<Uuid>,
) -> Result<Vec<Uuid>, AppError> {
    let bad = |message: &str| AppError::Generic(StatusCode::BAD_REQUEST, message.into());
    if replace_ids.iter().collect::<HashSet<_>>().len() != replace_ids.len() {
        return Err(bad("Duplicate replacement ID"));
    }
    let refs = values.iter().map(String::as_str).collect::<Vec<_>>();
    caps::check_per_value(&state.caps, &refs)?;
    let mut tx = state.db.begin().await?;
    // Serialize replacements in a project and hold membership through commit.
    let authorized: Option<Uuid> = sqlx::query_scalar("SELECT p.id FROM projects p JOIN user_project_relations upr ON upr.project_id=p.id WHERE p.id=$1 AND upr.user_id=$2 FOR UPDATE OF p FOR SHARE OF upr")
        .bind(project).bind(user_id).fetch_optional(&mut *tx).await?;
    if authorized.is_none() {
        return Err(Errors::Unauthorized.into());
    }
    let removed: Vec<String> = sqlx::query_scalar(
        "DELETE FROM variables WHERE project_id=$1 AND id=ANY($2) RETURNING value",
    )
    .bind(project)
    .bind(&replace_ids)
    .fetch_all(&mut *tx)
    .await?;
    if removed.len() != replace_ids.len() {
        return Err(bad(
            "Replacement variables changed or do not belong to this project",
        ));
    }
    let (count, bytes): (i64,i64) = sqlx::query_as("SELECT count(*), COALESCE(sum(octet_length(value)),0)::bigint FROM variables WHERE project_id=$1")
        .bind(project).fetch_one(&mut *tx).await?;
    let added: i64 = values.iter().map(|v| v.len() as i64).sum();
    if state.caps.max_variables_per_project > 0
        && count + values.len() as i64 > state.caps.max_variables_per_project
    {
        return Err(bad("Project variable cap exceeded"));
    }
    if state.caps.max_project_bytes > 0 && bytes + added > state.caps.max_project_bytes {
        return Err(bad("Project size cap exceeded"));
    }
    caps::check_user_total_on(&state.caps, &mut tx, user_id, added).await?;
    caps::check_and_record_ip_on(&state.caps, &mut tx, ip, added).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar("INSERT INTO variables(id,project_id,value) SELECT gen_random_uuid(),$1,value FROM UNNEST($2::text[]) AS t(value) RETURNING id")
        .bind(project).bind(&values).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    #[sqlx::test]
    async fn failed_insert_rolls_back_deleted_values(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let project = project(&pool, owner).await;
        let id: Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'original') RETURNING id").bind(project).fetch_one(&pool).await.unwrap();
        sqlx::query(
            "ALTER TABLE variables ADD CONSTRAINT reject_test_value CHECK (value <> 'reject-me')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(replace_many(
            &state(pool.clone()),
            owner,
            "127.0.0.1".parse().unwrap(),
            project,
            vec!["reject-me".into()],
            vec![id]
        )
        .await
        .is_err());
        let value: String = sqlx::query_scalar("SELECT value FROM variables WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "original");
    }
    #[sqlx::test]
    async fn foreign_replacement_rolls_back_entire_batch(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let victim = user(&pool).await;
        let own = project(&pool, owner).await;
        let other = project(&pool, victim).await;
        let a: Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'original') RETURNING id").bind(own).fetch_one(&pool).await.unwrap();
        let b: Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'victim') RETURNING id").bind(other).fetch_one(&pool).await.unwrap();
        assert!(replace_many(
            &state(pool.clone()),
            owner,
            "127.0.0.1".parse().unwrap(),
            own,
            vec!["new".into()],
            vec![a, b]
        )
        .await
        .is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM variables WHERE id=ANY($1)")
            .bind(vec![a, b])
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
    #[sqlx::test]
    async fn authorized_update_succeeds_but_mixed_batch_is_atomic(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let project = project(&pool, owner).await;
        let id:Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'original') RETURNING id").bind(project).fetch_one(&pool).await.unwrap();
        let item = || Variable {
            id: id.to_string(),
            project_id: project.to_string(),
            value: "updated".into(),
        };
        assert!(update_many(
            &state(pool.clone()),
            owner,
            "127.0.0.1".parse().unwrap(),
            vec![
                item(),
                Variable {
                    id: Uuid::new_v4().to_string(),
                    project_id: project.to_string(),
                    value: "missing".into()
                }
            ]
        )
        .await
        .is_err());
        let value: String = sqlx::query_scalar("SELECT value FROM variables WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "original");
        assert!(update_many(
            &state(pool.clone()),
            owner,
            "127.0.0.1".parse().unwrap(),
            vec![item()]
        )
        .await
        .is_ok());
        let value: String = sqlx::query_scalar("SELECT value FROM variables WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "updated");
    }
}
