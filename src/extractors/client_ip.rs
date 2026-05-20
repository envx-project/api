//! Extract the client IP, honoring X-Forwarded-For when behind a proxy
//! (Railway sets this). Falls back to peer SocketAddr.

use std::net::{IpAddr, Ipv4Addr};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::request::Parts,
};

use crate::error::AppError;

pub struct ClientIp(pub IpAddr);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _s: &S) -> Result<Self, Self::Rejection> {
        if let Some(value) = parts.headers.get("x-forwarded-for") {
            if let Ok(s) = value.to_str() {
                // X-Forwarded-For can be comma-separated; the leftmost entry
                // is the original client.
                if let Some(first) = s.split(',').next() {
                    if let Ok(ip) = first.trim().parse::<IpAddr>() {
                        return Ok(ClientIp(ip));
                    }
                }
            }
        }

        if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<std::net::SocketAddr>>()
        {
            return Ok(ClientIp(addr.ip()));
        }

        // Fall back to localhost rather than 500 — IP-based rate limiting
        // is best-effort; we don't want to break the request entirely if
        // we can't identify the client.
        Ok(ClientIp(IpAddr::V4(Ipv4Addr::LOCALHOST)))
    }
}
