use crate::*;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub mod invite;
pub mod project;
pub mod projects;
pub mod social;
pub mod user;
pub mod variables;

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(social::router(state.clone()))
        .nest("/project", project::router(state.clone()))
        .nest("/projects", projects::router(state.clone()))
        .nest("/user", user::router(state.clone()))
        .nest("/invite", invite::router(state.clone()))
        .nest("/variables", variables::router(state.clone()))
        .with_state(state)
}
