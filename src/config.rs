//! Runtime caps loaded from env vars.
//!
//! All caps default to the limits used on envx.sh production. Self-hosters
//! override by setting the env vars in their deployment (e.g. .env, docker
//! env, systemd). A value of `0` disables the cap.

use std::env;

#[derive(Debug, Clone)]
pub struct Caps {
    /// Reject a single variable whose ciphertext exceeds this many bytes.
    pub max_variable_bytes: u64,
    /// Reject an insert that would push a project past this many variables.
    pub max_variables_per_project: i64,
    /// Reject a write that would push a project past this many bytes of
    /// ciphertext.
    pub max_project_bytes: i64,
    /// Reject a write that would push a user's total across all of their
    /// projects past this many bytes.
    pub max_user_bytes: i64,
    /// Reject a write from an IP that has uploaded this many bytes in the
    /// last 24 hours.
    pub max_ip_bytes_per_day: i64,
}

impl Caps {
    pub fn from_env() -> Self {
        Self {
            max_variable_bytes: env_u64("ENVX_MAX_VARIABLE_BYTES", 128 * 1024),
            max_variables_per_project: env_i64("ENVX_MAX_VARIABLES_PER_PROJECT", 256),
            max_project_bytes: env_i64("ENVX_MAX_PROJECT_BYTES", 20 * 1024 * 1024),
            max_user_bytes: env_i64("ENVX_MAX_USER_BYTES", 100 * 1024 * 1024),
            max_ip_bytes_per_day: env_i64("ENVX_MAX_IP_BYTES_PER_DAY", 100 * 1024 * 1024),
        }
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_i64(name: &str, default: i64) -> i64 {
    nonnegative_cap(env::var(name).ok().as_deref(), default)
}

fn nonnegative_cap(value: Option<&str>, default: i64) -> i64 {
    value
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_or_invalid_caps_do_not_disable_limits() {
        assert_eq!(nonnegative_cap(Some("-1"), 256), 256);
        assert_eq!(nonnegative_cap(Some("invalid"), 256), 256);
        assert_eq!(nonnegative_cap(Some("0"), 256), 0);
        assert_eq!(nonnegative_cap(Some("42"), 256), 42);
    }
}
