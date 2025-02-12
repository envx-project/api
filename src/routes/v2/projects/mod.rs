pub(self) use crate::extractors::user::UserId;
pub(self) use crate::*;

mod list;
mod new;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/new", post(new::new_project_v2))
        .route("/{id}", get(list::list_projects_v2))
        .with_state(state)
}
