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
}

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

        Ok(Self {
            bind_address,
            database_url,
            storage_root,
            ingest_root,
            session_key,
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
