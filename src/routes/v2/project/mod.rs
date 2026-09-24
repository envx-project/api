use crate::structs::User;
use crate::*;
use crate::{extractors::user::UserId, helpers::project::user_in_project};
use axum::extract::Path;
use utoipa::ToSchema;

mod add_user;
mod delete;
mod info;
mod remove_users;
mod snapshot;
mod update;
mod variables;

pub const PROJECT_TAG: &str = "project";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(info::get_project_info_v2))
        .routes(routes!(delete::delete_project))
        .routes(routes!(update::update))
        .routes(routes!(add_user::add_user))
        .routes(routes!(remove_users::remove_users))
        .routes(routes!(variables::variables))
        .routes(routes!(snapshot::snapshot))
        .routes(routes!(snapshot::rewrap))
        .with_state(state)
}
