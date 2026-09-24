use crate::extractors::client_ip::ClientIp;
use crate::structs::Variable;

use super::*;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateManyBody {
    variables: Vec<Variable>,
}

#[utoipa::path(
    put,
    path = "/update-many",
    tag = VARIABLES_TAG,
    responses(
        (status = 200, description = "Project ID", body = Vec<String>),
        (status = 400, description = "Invalid public key"),
    ),
    security(
        ("bearer" = []),
    ),
)]
pub async fn update_many(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    ClientIp(ip): ClientIp,
    Json(body): Json<UpdateManyBody>,
) -> Result<Json<Vec<String>>, AppError> {
    Ok(Json(
        crate::helpers::variables::update_many(&state, user_id, ip, body.variables).await?,
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
        let result = update_many(
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
