use super::*;

mod new;

pub const INVITE_TAG: &str = "invite";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(new::new_invite))
        .with_state(state)
}
