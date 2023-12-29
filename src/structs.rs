use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Variable {
    pub id: String,
    pub value: String,
    pub project_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: String, // DateTime
    pub public_key: String,
}
