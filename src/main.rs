use axum::{
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use routes::{
    v2::{project::PROJECT_TAG, projects::PROJECTS_TAG, user::USER_TAG},
    *,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};
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
        (name = USER_TAG, description = "User API endpoints (for single user)"),
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let scheme = SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("Custom")
                .description(Some("Enter your Bearer token"))
                .build(),
        );

        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme("bearer", scheme);
        } else {
            openapi.components = Some(
                utoipa::openapi::ComponentsBuilder::new()
                    .security_scheme("bearer", scheme)
                    .build(),
            );
        }
    }
}

async fn init_router() -> anyhow::Result<Router> {
    tracing_subscriber::fmt().init();

    let db = db::db().await?;

    let state = AppState { db: Arc::new(db) };

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        // variables
        .route("/variable/new", post(variables::new_variable))
        .route("/variable/{variable_id}", get(variables::get_variable))
        .route(
            "/variables/{variable_id}",
            delete(variables::delete_variable),
        )
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
        .route("/project/{project_id}", get(projects::get_project_info))
        .route(
            "/project/{project_id}/variables",
            get(projects::get_project_variables),
        )
        .route("/project/{project_id}/add-user", post(projects::add_user))
        .route(
            "/project/{project_id}/remove-user",
            post(projects::remove_user),
        )
        // users
        .route("/user/new", post(user::new_user))
        .route("/user/{user_id}", get(user::get_user))
        // lowkey useless
        .route("/user/{user_id}/variables", get(user::get_all_variables))
        // miscellaneous
        .route("/", get(index::hello_world))
        .route("/test-auth", post(test_auth::test_auth))
        .routes(routes!(well_known::health_check))
        .nest("/v2/", routes::v2::router(state.clone()))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        .split_for_parts();

    std::fs::write("./openapi.json", api.to_json()?)?;

    let router = router.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", api));

    Ok(router)
}
