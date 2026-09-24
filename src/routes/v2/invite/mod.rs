use super::*;

mod accept;
mod new;

pub const INVITE_TAG: &str = "invite";

pub fn router(state: AppState) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(new::new_invite))
        .routes(routes!(accept::accept_invite))
        .routes(routes!(accept::prepare_invite))
        .with_state(state)
}
