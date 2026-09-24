use crate::error::{AppError, Errors};
use pgp::composed::{Deserializable, SignedPublicKey};

pub fn validate_public_key(value: &str) -> Result<(), AppError> {
    if value.len() > 128 * 1024 {
        return Err((
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            "Public key exceeds 128 KiB",
        )
            .into());
    }
    let (key, _) = SignedPublicKey::from_string(value)
        .map_err(|_| AppError::Error(Errors::InvalidPublicKey))?;
    key.verify()
        .map_err(|_| AppError::Error(Errors::InvalidPublicKey))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_oversized_keys() {
        assert!(validate_public_key("not a key").is_err());
        assert!(validate_public_key(&"x".repeat(128 * 1024 + 1)).is_err());
    }
}
