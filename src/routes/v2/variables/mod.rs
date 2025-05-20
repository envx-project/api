pub(self) use crate::structs::User;
pub(self) use crate::*;
pub(self) use crate::{extractors::user::UserId, helpers::project::user_in_project};
pub(self) use axum::extract::Path;
pub(self) use utoipa::ToSchema;
pub(self) use uuid::Uuid as UuidValidator;

mod delete;
mod get;
mod set_many;
mod update_many;

pub const VARIABLES_TAG: &str = "variables";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(delete::delete))
        .routes(routes!(get::get))
        .routes(routes!(set_many::set_many))
        .routes(routes!(update_many::update_many))
        .with_state(state)
}
