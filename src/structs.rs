use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct StrippedUser {
    pub id: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize)]
pub struct PartialKey {
    pub id: String,
    pub value: String,
    pub project_id: String,
    pub created_at: String,
}
