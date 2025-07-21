use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;

struct InviteState {
    inviter_key: String,
    inviter_socket: Option<SplitSink<WebSocket, Message>>,
    invitee_key: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: DB,
    pub invites: Arc<Mutex<HashMap<String, InviteState>>>,
}

pub type DB = Arc<sqlx::Pool<sqlx::Postgres>>;
