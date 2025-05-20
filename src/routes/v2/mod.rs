pub(self) use crate::*;
pub(self) use utoipa::ToSchema;
pub(self) use utoipa_axum::router::OpenApiRouter;
pub(self) use utoipa_axum::routes;

pub mod invite;
pub mod project;
pub mod projects;
pub mod user;
pub mod variables;

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .nest("/project", project::router(state.clone()))
        .nest("/projects", projects::router(state.clone()))
        .nest("/user", user::router(state.clone()))
        .nest("/invite", invite::router(state.clone()))
        .nest("/variables", variables::router(state.clone()))
        .with_state(state)
}
