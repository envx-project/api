use std::sync::Arc;

use crate::{
    error::{AppError, Errors},
    state::{AppState, DB},
    utils::rpgp::verify_signature,
};
use anyhow::bail;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use pgp::{composed::message::Message, Deserializable};

use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

use serde::Deserialize;

pub struct UserId(String);

#[derive(Deserialize, Debug)]
struct Token {
    token: String,
    signature: String,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}:{}", self.token, self.signature))?;

        Ok(())
    }
}

impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,       // required by FromRequest
    AppState: FromRef<S>, // required by FromRequest
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, s: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(s);
        // Extract the Authorization header
        let headers = parts.headers.clone();
        let auth_header = headers.get(header::AUTHORIZATION);

        match auth_header {
            Some(auth_header) => {
                let auth_header = auth_header.to_str().unwrap_or("");
                let auth_token = auth_header.trim_start_matches("Bearer ");

                let formatted_token = match serde_json::from_str::<Token>(auth_token) {
                    Ok(formatted_token) => formatted_token,
                    Err(_) => {
                        return Err(AppError::Error(Errors::Unauthorized));
                    }
                };

                let auth_token = &formatted_token.to_string();

                // parse content into a UTC datetime
                let user_id = match validate_challenge(auth_token, state.db).await {
                    Ok(user_id) => user_id,
                    Err(_) => {
                        return Err(AppError::Error(Errors::Unauthorized));
                    }
                };

                Ok(UserId(user_id.to_string()))
            }
            None => Err(AppError::Error(Errors::Unauthorized)),
        }
    }
}

async fn validate_challenge(challenge: &str, db: DB) -> anyhow::Result<Arc<String>> {
    let (user_id, challenge) = match challenge.split_once(':') {
        Some((user_id, challenge)) => (user_id, challenge),
        None => bail!("Invalid challenge"),
    };

    let (signed_challenge, _) = Message::from_string(challenge)?;
    let Some(content) = signed_challenge.get_content()? else {
        bail!("No content in signed challenge")
    };

    let challenge = std::str::from_utf8(&content)?;
    let challenge: DateTime<Utc> = challenge.parse()?;

    // check to make sure its not more than 10 minutes old
    let now = Utc::now();
    let diff = now.signed_duration_since(challenge);

    if diff.num_minutes() > 10 {
        bail!("Challenge is too old")
    }

    let user_pubkey = sqlx::query!(
        "SELECT public_key FROM users WHERE id = $1",
        Uuid::parse_str(user_id)?
    )
    .fetch_one(&*db)
    .await?
    .public_key;

    let verified = verify_signature(signed_challenge, user_pubkey)?;

    if !verified {
        bail!("Invalid signature")
    }

    Ok(Arc::new(user_id.to_string()))
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)?;

        Ok(())
    }
}

impl UserId {
    pub fn to_uuid(&self) -> Uuid {
        Uuid::parse_str(&self.0).unwrap()
    }
}
