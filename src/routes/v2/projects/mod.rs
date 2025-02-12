pub(self) use crate::extractors::user::UserId;
pub(self) use crate::*;
pub(self) use utoipa::ToSchema;

mod list;
mod new;

pub const PROJECTS_TAG: &str = "projects";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(new::new_project_v2))
        .routes(routes!(list::list_projects_v2))
        .with_state(state)
}
