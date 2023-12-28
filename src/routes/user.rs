use rocket::serde::{json::Json, Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
pub struct NewUserResponse {
    id: String,
    username: String,
}

#[derive(Serialize, Deserialize)]
pub struct GetUserResponse {
    id: String,
    public_key: String,
}

#[derive(Serialize, Deserialize)]
pub struct NewUserBody {
    username: String,
    public_key: String,
}

#[post("/user/new", format = "application/json", data = "<body>")]
pub async fn new_user(body: Json<NewUserBody>) -> Json<NewUserResponse> {
    let db = crate::db::db().await.unwrap();

    let user = sqlx::query!(
        "INSERT INTO users (username, public_key) VALUES ($1, $2) RETURNING id",
        body.username.clone(),
        body.public_key
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let string_id = user.id.to_string();

    Json(NewUserResponse {
        id: string_id,
        username: body.username.clone(),
    })
}

#[get("/user/<id>")]
pub async fn get_user(id: String) -> Json<GetUserResponse> {
    let db = crate::db::db().await.unwrap();

    let uuid_id = Uuid::parse_str(&id).unwrap();

    let user = sqlx::query!("SELECT id, public_key FROM users WHERE id = $1", uuid_id)
        .fetch_one(&db)
        .await
        .unwrap();

    Json(GetUserResponse {
        id: user.id.to_string(),
        public_key: user.public_key,
    })
}
