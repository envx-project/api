pub(self) use crate::structs::User;
pub(self) use crate::{extractors::user::UserId, helpers::project::user_in_project};
pub(self) use crate::{utils::uuid::UuidHelpers, *};
pub(self) use axum::extract::Path;
pub(self) use uuid::Uuid as UuidValidator;

mod info;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{id}", get(info::get_project_info_v2))
        .with_state(state)
}
