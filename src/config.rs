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
    pub disable_signup_after_first_user: bool,
    pub invite_expiration_seconds: i64,
}

const LOCAL_DEV_ENV_VAR: &str = "PAPYRD_LOCAL_DEV";
const LOCAL_DEV_UNSAFE_VALUE: &str = "enable-unsafe-development-environment";
const DISABLE_SIGNUP_AFTER_FIRST_USER_ENV_VAR: &str = "PAPYRD_DISABLE_SIGNUP_AFTER_FIRST_USER";
const INVITE_EXPIRATION_SECONDS_ENV_VAR: &str = "PAPYRD_INVITE_EXPIRATION_SECONDS";
const DEFAULT_INVITE_EXPIRATION_SECONDS: i64 = 24 * 60 * 60;

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
        let disable_signup_after_first_user = disable_signup_after_first_user_from_env(
            env::var(DISABLE_SIGNUP_AFTER_FIRST_USER_ENV_VAR)
                .ok()
                .as_deref(),
        )?;
        let invite_expiration_seconds = invite_expiration_seconds_from_env(
            env::var(INVITE_EXPIRATION_SECONDS_ENV_VAR).ok().as_deref(),
        )?;

        Ok(Self {
            bind_address,
            database_url,
            storage_root,
            ingest_root,
            session_key,
            session_cookie_secure,
            disable_signup_after_first_user,
            invite_expiration_seconds,
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
    #[error(
        "invalid PAPYRD_DISABLE_SIGNUP_AFTER_FIRST_USER environment variable; expected true or false"
    )]
    InvalidDisableSignupAfterFirstUser,
    #[error(
        "invalid PAPYRD_INVITE_EXPIRATION_SECONDS environment variable; expected a positive integer"
    )]
    InvalidInviteExpirationSeconds,
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

fn disable_signup_after_first_user_from_env(value: Option<&str>) -> Result<bool, ConfigError> {
    match value.map(str::trim) {
        None => Ok(true),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(ConfigError::InvalidDisableSignupAfterFirstUser),
    }
}

fn invite_expiration_seconds_from_env(value: Option<&str>) -> Result<i64, ConfigError> {
    match value.map(str::trim) {
        None => Ok(DEFAULT_INVITE_EXPIRATION_SECONDS),
        Some(value) => value
            .parse::<i64>()
            .ok()
            .filter(|seconds| *seconds > 0)
            .ok_or(ConfigError::InvalidInviteExpirationSeconds),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigError, DEFAULT_INVITE_EXPIRATION_SECONDS, disable_signup_after_first_user_from_env,
        invite_expiration_seconds_from_env, secure_session_cookie_from_local_dev_env,
    };

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

    #[test]
    fn signup_after_first_user_is_disabled_by_default() {
        assert!(disable_signup_after_first_user_from_env(None).unwrap());
    }

    #[test]
    fn signup_after_first_user_can_be_disabled() {
        assert!(disable_signup_after_first_user_from_env(Some("true")).unwrap());
    }

    #[test]
    fn signup_after_first_user_can_be_left_enabled_explicitly() {
        assert!(!disable_signup_after_first_user_from_env(Some("false")).unwrap());
    }

    #[test]
    fn signup_after_first_user_rejects_invalid_values() {
        assert!(matches!(
            disable_signup_after_first_user_from_env(Some("1")),
            Err(ConfigError::InvalidDisableSignupAfterFirstUser)
        ));
    }

    #[test]
    fn invite_expiration_defaults_to_one_day() {
        assert_eq!(
            invite_expiration_seconds_from_env(None).unwrap(),
            DEFAULT_INVITE_EXPIRATION_SECONDS
        );
    }

    #[test]
    fn invite_expiration_can_be_configured() {
        assert_eq!(
            invite_expiration_seconds_from_env(Some("3600")).unwrap(),
            3600
        );
    }

    #[test]
    fn invite_expiration_rejects_invalid_values() {
        assert!(matches!(
            invite_expiration_seconds_from_env(Some("0")),
            Err(ConfigError::InvalidInviteExpirationSeconds)
        ));
        assert!(matches!(
            invite_expiration_seconds_from_env(Some("-1")),
            Err(ConfigError::InvalidInviteExpirationSeconds)
        ));
        assert!(matches!(
            invite_expiration_seconds_from_env(Some("tomorrow")),
            Err(ConfigError::InvalidInviteExpirationSeconds)
        ));
    }
}
