use crate::{
    error::{AppError, Errors},
    state::{AppState, DB},
};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
};
use pgp::composed::{Deserializable, Message, SignedPublicKey};

use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

use serde::Deserialize;

pub struct UserId(pub Uuid);

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
        let auth_header = &parts.headers.get(header::AUTHORIZATION);

        match auth_header {
            Some(auth_header) => {
                let auth_header = auth_header.to_str().unwrap_or("");
                let auth_token = auth_header.trim_start_matches("Bearer ");

                let formatted_token = match serde_json::from_str::<Token>(auth_token) {
                    Ok(formatted_token) => formatted_token,
                    Err(_) => {
                        return Err(AppError::Generic(
                            StatusCode::UNAUTHORIZED,
                            "Invalid token".into(),
                        ));
                    }
                };

                let auth_token = &formatted_token.to_string();

                // parse content into a UTC datetime
                let user_id = match validate_challenge(auth_token, state.db).await {
                    Ok(user_id) => user_id,
                    Err(e) => match e {
                        ChallengeError::InvalidChallenge => {
                            return Err((StatusCode::BAD_REQUEST, "Invalid challenge").into())
                        }
                        ChallengeError::InvalidSignature => {
                            return Err((StatusCode::UNAUTHORIZED, "Invalid signature").into())
                        }
                        ChallengeError::NoContent => {
                            return Err((StatusCode::UNAUTHORIZED, "No content").into())
                        }
                        ChallengeError::TooOld => {
                            return Err((StatusCode::UNAUTHORIZED, "Too old").into())
                        }
                        ChallengeError::TooYoung => {
                            return Err((StatusCode::UNAUTHORIZED, "Too young").into())
                        }
                        ChallengeError::ChronoParseError(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e.into())))
                        }
                        ChallengeError::PgpError(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e.into())))
                        }
                        ChallengeError::SqlxError(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e.into())))
                        }
                        ChallengeError::Utf8Error(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e.into())))
                        }
                        ChallengeError::UuidError(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e.into())))
                        }
                        ChallengeError::Generic(e) => {
                            return Err(AppError::Error(Errors::InternalServerError(e)))
                        }
                    },
                };

                Ok(UserId(user_id))
            }
            None => Err(AppError::Error(Errors::Unauthorized)),
        }
    }
}

enum ChallengeError {
    InvalidChallenge,
    InvalidSignature,
    NoContent,
    TooOld,
    TooYoung,
    Generic(anyhow::Error),
    ChronoParseError(chrono::ParseError),
    PgpError(pgp::errors::Error),
    SqlxError(sqlx::Error),
    Utf8Error(std::str::Utf8Error),
    UuidError(uuid::Error),
}

impl From<anyhow::Error> for ChallengeError {
    fn from(e: anyhow::Error) -> Self {
        Self::Generic(e)
    }
}

impl From<chrono::ParseError> for ChallengeError {
    fn from(e: chrono::ParseError) -> Self {
        Self::ChronoParseError(e)
    }
}

impl From<pgp::errors::Error> for ChallengeError {
    fn from(e: pgp::errors::Error) -> Self {
        Self::PgpError(e)
    }
}

impl From<sqlx::Error> for ChallengeError {
    fn from(e: sqlx::Error) -> Self {
        Self::SqlxError(e)
    }
}

impl From<std::str::Utf8Error> for ChallengeError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::Utf8Error(e)
    }
}

impl From<uuid::Error> for ChallengeError {
    fn from(e: uuid::Error) -> Self {
        Self::UuidError(e)
    }
}

async fn validate_challenge(challenge: &str, db: DB) -> Result<Uuid, ChallengeError> {
    let (user_id, challenge) = match challenge.split_once(':') {
        Some((user_id, challenge)) => (Uuid::parse_str(user_id)?, challenge),
        None => Err(ChallengeError::InvalidChallenge)?,
    };

    let (mut signed_challenge, _) = Message::from_string(challenge)?;

    let content = signed_challenge.as_data_string().unwrap();

    let challenge: DateTime<Utc> = content.parse()?;

    // check to make sure its not more than 10 minutes old
    let diff = Utc::now().signed_duration_since(challenge);
    if diff.num_minutes() > 10 {
        Err(ChallengeError::TooOld)?
    }
    if diff.num_seconds() < 0 {
        Err(ChallengeError::TooYoung)?
    }

    let user_pubkey = sqlx::query!("SELECT public_key FROM users WHERE id = $1", user_id)
        .fetch_one(&*db)
        .await?
        .public_key;

    let verified = signed_challenge
        .verify(&SignedPublicKey::from_string(&user_pubkey)?.0)
        .is_ok();

    if !verified {
        Err(ChallengeError::InvalidSignature)?
    }

    Ok(user_id)
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())?;

        Ok(())
    }
}
