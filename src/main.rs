use axum::{
    routing::{get, post},
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

    let app = init_router().await?;

    let listener = tokio::net::TcpListener::bind("localhost:3000").await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn init_router() -> anyhow::Result<Router> {
    let db = db::db().await?;

    let state = AppState { db: Arc::new(db) };

    let router: Router = Router::new()
        // variables
        .route("/variable/new", post(variables::new_variable))
        .route("/variable/:id", get(variables::get_variable))
        .route("/variables/set-many", post(variables::set_many_variables))
        // projects
        .route("/project/:id", get(projects::get_project_info))
        .route(
            "/project/:id/variables",
            get(projects::get_project_variables),
        )
        // users
        .route("/user/new", post(user::new_user))
        // miscelaneous
        .route("/hello", get(index::hello_world))
        .route("/test-auth", post(test_auth::test_auth))
        .with_state(state);

    Ok(router)
}
