use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use dotenv::dotenv;

//#region mod
mod routes;
use routes::*;

mod db;
mod error;
mod extractors;
mod helpers;
mod state;
mod structs;
mod traits;
mod utils;
//#endregion

//#region global use
pub(crate) use anyhow::Context;
pub(crate) use axum::extract::{Json, State};
pub(crate) use error::{AnyhowError, AppError, Errors};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use state::AppState;
//#endregion

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
        .route("/variable/new", post(variables::new_variable))
        .route("/variable/:id", get(variables::get_variable))
        .route("/project/:id", get(projects::get_project_info))
        .route("/user/new", post(user::new_user))
        .route("/hello", get(index::hello_world))
        .route("/test-auth", post(test_auth::test_auth))
        .with_state(state);

    Ok(router)
}
