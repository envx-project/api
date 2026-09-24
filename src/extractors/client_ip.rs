//! Forwarding headers are trusted only when explicitly enabled by the operator.
use crate::error::AppError;
use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, HeaderMap},
};
use std::net::{IpAddr, Ipv4Addr};

pub struct ClientIp(pub IpAddr);

fn client_ip(headers: &HeaderMap, peer: IpAddr, trust_proxy: bool) -> IpAddr {
    if trust_proxy && headers.get_all("x-real-ip").iter().count() == 1 {
        if let Some(ip) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
        {
            return ip;
        }
    }
    peer
}

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _s: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|addr| addr.0.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let trust_proxy = std::env::var("ENVX_TRUST_PROXY").as_deref() == Ok("true");
        Ok(ClientIp(client_ip(&parts.headers, peer, trust_proxy)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarding_requires_explicit_trust_and_single_ip() {
        let peer = "127.0.0.1".parse().unwrap();
        let remote: IpAddr = "192.0.2.1".parse().unwrap();
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.1, 192.0.2.1".parse().unwrap());
        assert_eq!(client_ip(&h, peer, true), peer);
        h.insert("x-real-ip", "192.0.2.1".parse().unwrap());
        assert_eq!(client_ip(&h, peer, false), peer);
        assert_eq!(client_ip(&h, peer, true), remote);
        h.append("x-real-ip", "203.0.113.1".parse().unwrap());
        assert_eq!(client_ip(&h, peer, true), peer);
        h.insert("x-real-ip", "192.0.2.1, 203.0.113.1".parse().unwrap());
        assert_eq!(client_ip(&h, peer, true), peer);
    }
}
