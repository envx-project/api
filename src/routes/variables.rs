use crate::db;
use crate::*;
use rocket::serde::{json::Json, Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Body {
    project_id: String,
    value: String,
    encrypted: bool,
}

#[post("/variable/new", format = "application/json", data = "<body>")]
pub async fn new_variable(
    body: Json<Body>,
) -> Result<String, rocket::response::status::BadRequest<String>> {
    let db = db::db().await.unwrap();

    // check to make sure value isn't >1M
    if body.value.len() > 1_000_000 {
        return Err(rocket::response::status::BadRequest(
            "value cannot be greater than 1M".to_string(),
        ));
    }

    let test = sqlx::query!(
        "INSERT INTO variables (value, encrypted) VALUES ($1, $2) RETURNING id",
        body.value,
        body.encrypted
    )
    .fetch_one(&db)
    .await
    .unwrap();

    Ok(String::from(format!("test: {}", test.id)))
}

#[get("/variable/<id>")]
pub async fn get_variable(
    id: String,
) -> Result<String, rocket::response::status::NotFound<String>> {
    let db = db::db().await.unwrap();

    let uuid_id = Uuid::parse_str(&id).unwrap();

    let variable = sqlx::query!(
        "SELECT id, value, project_id FROM variables WHERE id = $1",
        uuid_id
    )
    .fetch_one(&db)
    .await
    .unwrap();

    if variable.project_id.is_some() {}

    Ok(String::from(format!("variable: {}", variable.id)))
}
