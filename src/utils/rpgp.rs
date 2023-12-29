use anyhow::{Context, Ok, Result};
use pgp::composed::message::Message;
use pgp::{composed::signed_key::*, Deserializable};

// use crypto_hash::{hex_digest, Algorithm};
// use hex::ToHex;
// use pgp::types::public;
// use pgp::{composed, crypto, types::SecretKeyTrait};
// use rand::prelude::*;
// use smallvec::*;

pub fn verify_signature(message: Message, public_key: String) -> Result<bool> {
    let public_key = SignedPublicKey::from_string(&public_key)
        .context("invalid public key")?
        .0;
    let verified = message.verify(&public_key);
    Ok(verified.is_ok())
}
