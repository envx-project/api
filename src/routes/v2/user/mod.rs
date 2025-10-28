 use crate::*;
 use crate::extractors::user::UserId;
 use utoipa::ToSchema;
 use uuid::Uuid;

mod get;
mod get_many;
mod new;

pub const USER_TAG: &str = "user";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(new::new_user_v2))
        .routes(routes!(get::get_user_v2))
        .routes(routes!(get_many::get_many_users))
        .with_state(state)
}
