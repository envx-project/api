use crate::db;
use crate::structs::Variable;
use crate::*;
use anyhow::Ok;
use rocket::serde::{json::Json, Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Body {
    user_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct NewProjectReturnType {
    project_id: String,
}

#[post("/project/new", format = "application/json", data = "<body>")]
pub async fn new_project(body: Json<Body>) -> Json<NewProjectReturnType> {
    async fn insert_project(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: String,
    ) -> anyhow::Result<String> {
        let project = sqlx::query!("INSERT INTO projects DEFAULT VALUES RETURNING id")
            .fetch_one(&mut **transaction)
            .await?;

        sqlx::query!(
            "INSERT INTO user_project_relations (user_id, project_id) VALUES ($1, $2)",
            Uuid::parse_str(&user_id).unwrap(),
            project.id
        )
        .execute(&mut **transaction)
        .await?;

        Ok(project.id.to_string())
    }

    let db = db::db().await.unwrap();

    let mut transaction = db.begin().await.unwrap();

    let project_id = insert_project(&mut transaction, body.user_id.clone())
        .await
        .unwrap();

    transaction.commit().await.unwrap();

    Json(NewProjectReturnType { project_id })
}

#[derive(Serialize, Deserialize)]
struct AddUserBody {
    user_id: String,
}

#[derive(Serialize, Deserialize)]
struct AddUserReturnType {
    project_id: String,
}

#[post(
    "/project/<project_id>/add_user",
    format = "application/json",
    data = "<body>"
)]
pub async fn add_user_to_project(
    project_id: String,
    body: Json<AddUserBody>,
) -> Json<AddUserReturnType> {
    let db = db::db().await.unwrap();

    sqlx::query!(
        "INSERT INTO user_project_relations (user_id, project_id) VALUES ($1, $2)",
        Uuid::parse_str(&body.user_id).unwrap(),
        Uuid::parse_str(&project_id).unwrap()
    )
    .execute(&db)
    .await
    .unwrap();

    Json(AddUserReturnType {
        project_id: project_id.clone(),
    })
}

#[get("/project/<project_id>/variables")]
pub async fn get_variables(project_id: String) -> Json<Vec<Variable>> {
    let db = db::db().await.unwrap();

    let variables = sqlx::query!(
        "SELECT id, value, encrypted FROM variables WHERE project_id = $1",
        Uuid::parse_str(&project_id).unwrap()
    )
    .fetch_all(&db)
    .await
    .unwrap();

    let mut encrypted_variables: Vec<Variable> = Vec::new();

    for variable in variables {
        encrypted_variables.push(Variable {
            id: variable.id.to_string(),
            value: variable.value,
            encrypted: variable.encrypted.unwrap_or(false),
            project_id: project_id.clone(),
        });
    }

    Json(encrypted_variables)
}
