use pgp::{Deserializable, SignedPublicKey};

use crate::{extractors::user::UserId, *};

#[derive(Serialize, Deserialize)]
pub struct NewUserBody {
    username: String,
    public_key: String,
}

#[derive(Serialize, Deserialize)]
pub struct NewUserReturnType {
    id: String,
}

pub async fn new_user(
    State(state): State<AppState>,
    Json(body): Json<NewUserBody>,
) -> Result<Json<NewUserReturnType>, AppError> {
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

    Ok(Json(NewUserReturnType {
        id: user.id.to_string(),
    }))
}
