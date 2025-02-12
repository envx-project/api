use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

pub mod project;
pub mod projects;

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .nest("/project", project::router(state.clone()))
        .nest("/projects", projects::router(state.clone()))
        .with_state(state)
}
