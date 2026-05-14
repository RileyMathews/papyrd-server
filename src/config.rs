use std::{env, net::SocketAddr, path::PathBuf};

use axum_extra::extract::cookie::Key;
use sha2::{Digest, Sha512};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub storage_root: PathBuf,
    pub ingest_root: PathBuf,
    pub session_key: Key,
    pub session_cookie_secure: bool,
}

const LOCAL_DEV_ENV_VAR: &str = "PAPYRD_LOCAL_DEV";
const LOCAL_DEV_UNSAFE_VALUE: &str = "enable-unsafe-development-environment";

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_address = match env::var("PAPYRD_BIND_ADDRESS") {
            Ok(value) => value.parse()?,
            Err(_) => default_bind_address(),
        };

        let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        let storage_root = env::var("PAPYRD_STORAGE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("storage"));
        let ingest_root = env::var("PAPYRD_INGEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| storage_root.join("ingest"));
        let session_secret =
            env::var("PAPYRD_SESSION_SECRET").map_err(|_| ConfigError::MissingSessionSecret)?;
        let session_key = Key::from(&derive_session_key(&session_secret));
        let session_cookie_secure =
            secure_session_cookie_from_local_dev_env(env::var(LOCAL_DEV_ENV_VAR).ok().as_deref());

        Ok(Self {
            bind_address,
            database_url,
            storage_root,
            ingest_root,
            session_key,
            session_cookie_secure,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required DATABASE_URL environment variable")]
    MissingDatabaseUrl,
    #[error("missing required PAPYRD_SESSION_SECRET environment variable")]
    MissingSessionSecret,
    #[error("invalid PAPYRD_BIND_ADDRESS")]
    InvalidBindAddress(#[from] std::net::AddrParseError),
}

fn default_bind_address() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 3000))
}

fn derive_session_key(secret: &str) -> [u8; 64] {
    let digest = Sha512::digest(secret.as_bytes());
    let mut key = [0_u8; 64];
    key.copy_from_slice(&digest);
    key
}

fn secure_session_cookie_from_local_dev_env(local_dev_value: Option<&str>) -> bool {
    local_dev_value != Some(LOCAL_DEV_UNSAFE_VALUE)
}

#[cfg(test)]
mod tests {
    use super::secure_session_cookie_from_local_dev_env;

    #[test]
    fn session_cookie_is_secure_by_default() {
        assert!(secure_session_cookie_from_local_dev_env(None));
    }

    #[test]
    fn session_cookie_stays_secure_for_non_matching_local_dev_value() {
        assert!(secure_session_cookie_from_local_dev_env(Some("true")));
        assert!(secure_session_cookie_from_local_dev_env(Some(
            "enable-unsafe-development-environment "
        )));
    }

    #[test]
    fn session_cookie_secure_can_be_disabled_for_explicit_local_dev_value() {
        assert!(!secure_session_cookie_from_local_dev_env(Some(
            "enable-unsafe-development-environment"
        )));
    }
}
