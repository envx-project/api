use crate::{
    extractors::user::UserId,
    structs::{PartialKey, StrippedUser},
    utils::uuid::UuidHelpers,
    *,
};
use axum::extract::Path;
use pgp::{Deserializable, SignedPublicKey};
use uuid::Uuid as UuidValidator;

#[derive(Serialize, Deserialize)]
pub struct NewUserBody {
    username: String,
    public_key: String,
}

pub async fn new_user(
    State(state): State<AppState>,
    Json(body): Json<NewUserBody>,
) -> Result<String, AppError> {
    // public key validation
    match SignedPublicKey::from_string(&body.public_key) {
        Ok(_) => {}
        Err(_) => {
            return Err(AppError::Error(Errors::InvalidPublicKey));
        }
    }

    let user = sqlx::query!(
        "INSERT INTO users (username, public_key) VALUES ($1, $2) RETURNING id",
        body.username,
        body.public_key
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to insert user")?;

    Ok(user.id.to_string())
}

pub async fn get_user(
    State(state): State<AppState>,
    _: UserId,
    Path(id): Path<UuidValidator>,
) -> Result<Json<StrippedUser>, AppError> {
    let user = sqlx::query_as!(
        StrippedUser,
        "SELECT id, public_key FROM users WHERE id = $1",
        id.to_sqlx()
    )
    .fetch_one(&*state.db)
    .await
    .context("Failed to get user")?;

    Ok(Json(user))
}

pub async fn get_all_variables(
    State(state): State<AppState>,
    user_id: UserId,
) -> Result<Json<Vec<PartialKey>>, AppError> {
    let variables = sqlx::query!(
        "SELECT v.id, v.value, v.project_id, v.created_at
        FROM users u
        JOIN user_project_relations upr ON u.id = upr.user_id
        JOIN variables v ON upr.project_id = v.project_id
        WHERE u.id = $1",
        user_id.to_uuid()
    )
    .fetch_all(&*state.db)
    .await
    .context("Failed to get variables")?;

    let partial_keys = variables
        .iter()
        .map(|variable| PartialKey {
            id: variable.id.to_string(),
            value: variable.value.clone(),
            project_id: variable.project_id.to_string(),
            created_at: variable.created_at.to_string(),
        })
        .collect::<Vec<PartialKey>>();

    Ok(Json(partial_keys))
}
