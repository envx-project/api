use axum::{
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use routes::{
    v2::{project::PROJECT_TAG, projects::PROJECTS_TAG},
    *,
};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

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

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = PROJECTS_TAG, description = "Project API endpoints (for projects under each user)"),
        (name = PROJECT_TAG, description = "Project API endpoints (for single project)"),
    ),
)]
struct ApiDoc;

async fn init_router() -> anyhow::Result<Router> {
    let db = db::db().await?;

    // sqlx::migrate!().run(&db).await?;

    let state = AppState { db: Arc::new(db) };

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        // variables
        .route("/variable/new", post(variables::new_variable))
        .route("/variable/{id}", get(variables::get_variable))
        .route("/variables/{id}", delete(variables::delete_variable))
        .route("/variables/set-many", post(variables::set_many_variables))
        .route(
            "/variables/set-many/v2",
            post(variables::set_many_variables_v2),
        )
        .route(
            "/variables/update-many",
            post(variables::update_many_variables),
        )
        // projects
        .route("/projects/new", post(projects::new_project))
        .route("/projects", get(projects::list_projects))
        .route("/project/{id}", get(projects::get_project_info))
        .route(
            "/project/{id}/variables",
            get(projects::get_project_variables),
        )
        .route("/project/{id}/add-user", post(projects::add_user))
        .route("/project/{id}/remove-user", post(projects::remove_user))
        // users
        .route("/user/new", post(user::new_user))
        .route("/user/{id}", get(user::get_user))
        .route("/user/{id}/variables", get(user::get_all_variables))
        // miscellaneous
        .route("/", get(index::hello_world))
        .route("/test-auth", post(test_auth::test_auth))
        .routes(routes!(well_known::health_check))
        .nest("/v2/", routes::v2::router(state.clone()))
        .with_state(state)
        .split_for_parts();

    let router = router.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", api));

    Ok(router)
}
