use crate::extractors::client_ip::ClientIp;
use crate::structs::Variable;
use crate::traits::to_uuid::ToUuid;
use crate::*;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use axum::extract::Path;
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
pub struct NewVariableBody {
    project_id: String,
    value: String,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NewVariableReturnType {
    id: String,
}

pub async fn new_variable(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<NewVariableBody>,
) -> Result<Json<NewVariableReturnType>, AppError> {
    let ids = crate::helpers::variables::insert_many(
        &state,
        user_id,
        ip,
        body.project_id.to_uuid()?,
        vec![body.value],
        vec![body.tag.unwrap_or_default()],
    )
    .await?;
    Ok(Json(NewVariableReturnType {
        id: ids[0].to_string(),
    }))
}

#[derive(Serialize, Deserialize)]
pub struct SetManyBody {
    project_id: String,
    variables: Vec<String>,
    #[serde(default)]
    replace_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize)]
pub struct SetManyReturnType {
    id: String,
}

pub async fn set_many_variables(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<SetManyBody>,
) -> Result<Json<Vec<SetManyReturnType>>, AppError> {
    let ids = crate::helpers::variables::replace_many(
        &state,
        user_id,
        ip,
        body.project_id.to_uuid()?,
        body.variables,
        body.replace_ids,
    )
    .await?;
    Ok(Json(
        ids.into_iter()
            .map(|id| SetManyReturnType { id: id.to_string() })
            .collect(),
    ))
}

// literally what is the point of this method
pub async fn get_variable(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<String, AppError> {
    let variable = sqlx::query!(
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        variable_id
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get variable")?;

    if !user_in_project(user_id, variable.project_id, &state.db).await? {
        return Err(AppError::Error(Errors::Unauthorized));
    }

    Ok(format!("variable: {}", variable.id))
}

#[derive(Serialize, Deserialize)]
pub struct UpdateManyBody {
    variables: Vec<Variable>,
}

pub async fn update_many_variables(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<UpdateManyBody>,
) -> Result<Json<Vec<String>>, AppError> {
    Ok(Json(
        crate::helpers::variables::update_many(&state, user_id, ip, body.variables).await?,
    ))
}

pub async fn delete_variable(
    State(state): State<AppState>,
    Path(variable_id): Path<Uuid>,
    UserId(user_id): UserId,
) -> Result<(), AppError> {
    crate::helpers::variables::delete(&state, user_id, variable_id).await
}

#[derive(Serialize, Deserialize)]
pub struct V2VariableInput {
    value: String,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct V2SetManyBody {
    project_id: String,
    variables: Vec<V2VariableInput>,
}

#[derive(Serialize, Deserialize)]
pub struct V2SetManyReturnType {
    id: String,
}

pub async fn set_many_variables_v2(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<V2SetManyBody>,
) -> Result<Json<Vec<V2SetManyReturnType>>, AppError> {
    let (values, tags) = body
        .variables
        .into_iter()
        .map(|v| (v.value, v.tag.unwrap_or_default()))
        .unzip();
    let ids = crate::helpers::variables::insert_many(
        &state,
        user_id,
        ip,
        body.project_id.to_uuid()?,
        values,
        tags,
    )
    .await?;
    Ok(Json(
        ids.into_iter()
            .map(|id| V2SetManyReturnType { id: id.to_string() })
            .collect(),
    ))
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
    use crate::test_support::*;
    #[sqlx::test]
    async fn mismatched_project_cannot_overwrite_variable(pool: sqlx::PgPool) {
        let attacker = user(&pool).await;
        let victim = user(&pool).await;
        let own = project(&pool, attacker).await;
        let other = project(&pool, victim).await;
        let id: Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'original') RETURNING id").bind(other).fetch_one(&pool).await.unwrap();
        let result = update_many_variables(
            State(state(pool.clone())),
            UserId(attacker),
            ClientIp("127.0.0.1".parse().unwrap()),
            Json(UpdateManyBody {
                variables: vec![Variable {
                    id: id.to_string(),
                    project_id: own.to_string(),
                    value: "tampered".into(),
                }],
            }),
        )
        .await;
        assert!(
            result.is_err(),
            "must reject a variable belonging to another project"
        );
        let value: String = sqlx::query_scalar("SELECT value FROM variables WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "original");
    }
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    use crate::test_support::*;
    #[sqlx::test]
    async fn replacement_removes_old_values_only_on_success(pool: sqlx::PgPool) {
        let owner = user(&pool).await;
        let project = project(&pool, owner).await;
        let id: Uuid=sqlx::query_scalar("INSERT INTO variables(id,project_id,value) VALUES (gen_random_uuid(),$1,'original') RETURNING id").bind(project).fetch_one(&pool).await.unwrap();
        let body=serde_json::from_value(serde_json::json!({"project_id":project,"variables":["replacement"],"replace_ids":[id]})).unwrap();
        assert!(set_many_variables(
            State(state(pool.clone())),
            UserId(owner),
            ClientIp("127.0.0.1".parse().unwrap()),
            Json(body)
        )
        .await
        .is_ok());
        let values: Vec<String> =
            sqlx::query_scalar("SELECT value FROM variables WHERE project_id=$1")
                .bind(project)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(values, vec!["replacement"]);
    }
}
