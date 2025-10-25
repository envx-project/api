use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct Variable {
    pub id: String,
    pub value: String,
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: String, // DateTime
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct StrippedUser {
    pub id: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct PartialKey {
    pub id: String,
    pub value: String,
    pub project_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct ProjectInvite {
    pub id: String,
    pub project_id: String,
    pub author_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub verifier_argon2id: String,
    pub ciphertext: Option<String>,
    pub invited_id: Option<Uuid>,
}
