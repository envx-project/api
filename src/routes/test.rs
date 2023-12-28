use crate::db;
use crate::*;

#[get("/test")]
pub async fn test() -> Result<String, rocket::response::status::NotFound<String>> {
    let db = db::db().await.unwrap();

    // insert user into
    /*
        CREATE TABLE users (
        id SERIAL PRIMARY KEY,
        username VARCHAR(64) NOT NULL,
        email VARCHAR(128) NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    ); */

    // let user = sqlx::query!(
    //     "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
    //     "test",
    //     "test@example.com"
    // )
    // .fetch_one(&db)
    // .await
    // .unwrap();

    let test = sqlx::query!(
        "INSERT INTO variables (value) VALUES ($1) RETURNING id",
        "test"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    Ok(String::from(format!("test: {}", test.id)))
}
