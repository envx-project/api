pub(self) use crate::structs::User;
pub(self) use crate::{extractors::user::UserId, helpers::project::user_in_project};
pub(self) use crate::{utils::uuid::UuidHelpers, *};
pub(self) use axum::extract::Path;
pub(self) use utoipa::ToSchema;
pub(self) use uuid::Uuid as UuidValidator;

mod get;
mod new;

pub const USER_TAG: &str = "user";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(new::new_user_v2))
        .routes(routes!(get::get_user_v2))
        .with_state(state)
}
