use axum::Router;

use crate::AppState;

pub mod project;
pub mod projects;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/project", project::router(state.clone()))
        .nest("/projects", projects::router(state.clone()))
        .with_state(state)
}
