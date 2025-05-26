pub(self) use crate::structs::User;
pub(self) use crate::*;
pub(self) use crate::{extractors::user::UserId, helpers::project::user_in_project};
pub(self) use axum::extract::Path;
pub(self) use utoipa::ToSchema;

mod add_user;
mod info;
mod remove_users;
mod update;
mod variables;

pub const PROJECT_TAG: &str = "project";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(info::get_project_info_v2))
        .routes(routes!(update::update))
        // TODO
        .routes(routes!(add_user::add_user))
        .routes(routes!(remove_users::remove_users))
        .routes(routes!(variables::variables))
        .with_state(state)
}
