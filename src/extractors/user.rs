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
                let auth_token =
                    bearer_payload(auth_header).ok_or(AppError::Error(Errors::Unauthorized))?;

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
                        ChallengeError::SqlxError(e) if !matches!(e, sqlx::Error::RowNotFound) => {
                            return Err(AppError::Error(Errors::SqlxError(e)));
                        }
                        _ => return Err((StatusCode::UNAUTHORIZED, "Invalid credentials").into()),
                    },
                };

                Ok(UserId(user_id))
            }
            None => Err(AppError::Error(Errors::Unauthorized)),
        }
    }
}

#[allow(dead_code)]
enum ChallengeError {
    InvalidChallenge,
    InvalidSignature,
    TooOld,
    TooYoung,
    Generic(anyhow::Error),
    ChronoParseError(chrono::ParseError),
    PgpError(pgp::errors::Error),
    SqlxError(sqlx::Error),
    Utf8Error(std::str::Utf8Error),
    UuidError(uuid::Error),
    IoError(std::io::Error),
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

impl From<std::io::Error> for ChallengeError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

async fn validate_challenge(challenge: &str, db: DB) -> Result<Uuid, ChallengeError> {
    let (user_id, challenge) = match challenge.split_once(':') {
        Some((user_id, challenge)) => (Uuid::parse_str(user_id)?, challenge),
        None => Err(ChallengeError::InvalidChallenge)?,
    };

    let (mut signed_challenge, _) = Message::from_string(challenge)?;

    let content = signed_challenge.as_data_string()?;

    let challenge: DateTime<Utc> = content.parse()?;

    // check to make sure its not more than 10 minutes old
    let diff = Utc::now().signed_duration_since(challenge);
    if diff.num_seconds() > 10 * 60 {
        return Err(ChallengeError::TooOld);
    }
    if diff.num_seconds() < 0 {
        return Err(ChallengeError::TooYoung);
    }

    let user_pubkey = sqlx::query!("SELECT public_key FROM users WHERE id = $1", user_id)
        .fetch_one(&*db)
        .await?
        .public_key;

    let verified = signed_challenge
        .verify(&SignedPublicKey::from_string(&user_pubkey)?.0)
        .is_ok();

    if !verified {
        return Err(ChallengeError::InvalidSignature);
    }

    Ok(user_id)
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())?;

        Ok(())
    }
}

// Older released clients accidentally prefixed Bearer twice. Keep that exact
// compatibility case, but require an authentication scheme and reject repeats.
fn bearer_payload(value: &str) -> Option<&str> {
    let payload = value.strip_prefix("Bearer ")?;
    Some(payload.strip_prefix("Bearer ").unwrap_or(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn malformed_credentials_return_unauthorized_without_database_access() {
        use axum::{http::Request, response::IntoResponse};
        use std::sync::Arc;
        let state = AppState {
            db: Arc::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://invalid/unused")
                    .unwrap(),
            ),
            caps: Arc::new(crate::config::Caps::from_env()),
        };
        for value in [
            "{}".to_owned(),
            "Bearer not-json".to_owned(),
            r#"Bearer {"token":"not-a-uuid","signature":"not-pgp"}"#.to_owned(),
            format!(
                r#"Bearer {{"token":"{}","signature":"not-pgp"}}"#,
                Uuid::new_v4()
            ),
        ] {
            let (mut parts, _) = Request::builder()
                .header(header::AUTHORIZATION, value)
                .body(())
                .unwrap()
                .into_parts();
            let result = UserId::from_request_parts(&mut parts, &state).await;
            match result {
                Err(error) => assert_eq!(error.into_response().status(), StatusCode::UNAUTHORIZED),
                Ok(_) => panic!("malformed credentials accepted"),
            }
        }
    }

    #[test]
    fn bearer_scheme_required_with_legacy_compatibility() {
        assert_eq!(bearer_payload("Bearer {}"), Some("{}"));
        assert_eq!(bearer_payload("Bearer Bearer {}"), Some("{}"));
        assert_eq!(bearer_payload("{}"), None);
        assert_eq!(bearer_payload("Basic {}"), None);
        assert!(
            serde_json::from_str::<Token>(bearer_payload("Bearer Bearer Bearer {}").unwrap())
                .is_err()
        );
    }
}
