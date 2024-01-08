use axum::{
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use routes::*;
use std::sync::Arc;

//#region mod
mod db;
mod error;
mod extractors;
mod helpers;
mod routes;
mod state;
mod structs;
mod traits;
mod utils;

//#region global use
pub(crate) use anyhow::Context;
pub(crate) use axum::extract::{Json, State};
pub(crate) use error::{AnyhowError, AppError, Errors};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    let app = init_router().await?;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("listening on http://localhost:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn init_router() -> anyhow::Result<Router> {
    let db = db::db().await?;

    sqlx::migrate!().run(&db).await?;

    let state = AppState { db: Arc::new(db) };

    let router: Router = Router::new()
        // variables
        .route("/variable/new", post(variables::new_variable))
        .route("/variable/:id", get(variables::get_variable))
        .route("/variables/:id", delete(variables::delete_variable))
        .route("/variables/set-many", post(variables::set_many_variables))
        .route(
            "/variables/update-many",
            post(variables::update_many_variables),
        )
        // projects
        .route("/projects/new", post(projects::new_project))
        .route("/projects", get(projects::list_projects))
        .route("/project/:id", get(projects::get_project_info))
        .route(
            "/project/:id/variables",
            get(projects::get_project_variables),
        )
        .route("/project/:id/add-user", post(projects::add_user))
        // users
        .route("/user/new", post(user::new_user))
        .route("/user/:id", get(user::get_user))
        .route("/user/:id/variables", get(user::get_all_variables))
        // miscelaneous
        .route("/", get(index::hello_world))
        .route("/test-auth", post(test_auth::test_auth))
        // well-known
        .route("/.well-known/health-check", get(well_known::health_check))
        .with_state(state);

    Ok(router)
}
