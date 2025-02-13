pub(self) use crate::structs::User;
pub(self) use crate::{extractors::user::UserId, helpers::project::user_in_project};
pub(self) use crate::{utils::uuid::UuidHelpers, *};
pub(self) use axum::extract::Path;
pub(self) use utoipa::ToSchema;
pub(self) use uuid::Uuid as UuidValidator;

mod info;
mod update;

pub const PROJECT_TAG: &str = "project";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(info::get_project_info_v2))
        .routes(routes!(update::update_project_v2))
        .with_state(state)
}
