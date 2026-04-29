use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::http::HeaderValue;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{domain::user::User, error::AppError, repositories::users};

const SESSION_COOKIE_NAME: &str = "papyrd_session";

pub fn normalize_username(input: &str) -> Option<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_lowercase())
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AppError::PasswordHash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| AppError::PasswordHash)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn kosync_userkey(password: &str) -> String {
    format!("{:x}", md5::compute(password))
}

pub fn hash_kosync_userkey(userkey: &str) -> Result<String, AppError> {
    hash_password(userkey)
}

pub fn verify_kosync_userkey(userkey: &str, userkey_hash: &str) -> Result<bool, AppError> {
    verify_password(userkey, userkey_hash)
}

pub fn sign_in_jar(jar: PrivateCookieJar, user_id: Uuid) -> PrivateCookieJar {
    jar.add(
        Cookie::build((SESSION_COOKIE_NAME, user_id.to_string()))
            .path("/")
            .http_only(true),
    )
}

pub fn sign_out_jar(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.remove(Cookie::build((SESSION_COOKIE_NAME, "")).path("/"))
}

pub async fn current_user(db: &PgPool, jar: &PrivateCookieJar) -> Result<Option<User>, AppError> {
    let Some(cookie) = jar.get(SESSION_COOKIE_NAME) else {
        return Ok(None);
    };

    let Ok(user_id) = Uuid::parse_str(cookie.value()) else {
        return Ok(None);
    };

    users::find_user_by_id(db, user_id)
        .await
        .map_err(Into::into)
}

pub async fn basic_auth_user(
    db: &PgPool,
    authorization: Option<&HeaderValue>,
) -> Result<Option<User>, AppError> {
    let Some(authorization) = authorization else {
        return Ok(None);
    };

    let Ok(authorization) = authorization.to_str() else {
        return Ok(None);
    };

    let Some(encoded) = authorization.strip_prefix("Basic ") else {
        return Ok(None);
    };

    let Ok(decoded) = STANDARD.decode(encoded) else {
        return Ok(None);
    };

    let Ok(credentials) = String::from_utf8(decoded) else {
        return Ok(None);
    };

    let Some((username, password)) = credentials.split_once(':') else {
        return Ok(None);
    };

    let Some(normalized_username) = normalize_username(username) else {
        return Ok(None);
    };

    let Some(stored_user) =
        users::find_user_by_normalized_username(db, &normalized_username).await?
    else {
        return Ok(None);
    };

    if !verify_password(password, &stored_user.password_hash)? {
        return Ok(None);
    }

    Ok(Some(stored_user.user))
}

pub async fn kosync_auth_user(
    db: &PgPool,
    username_header: Option<&HeaderValue>,
    userkey_header: Option<&HeaderValue>,
) -> Result<Option<User>, AppError> {
    let Some(username) = header_value_str(username_header) else {
        return Ok(None);
    };
    let Some(userkey) = header_value_str(userkey_header) else {
        return Ok(None);
    };
    let Some(normalized_username) = normalize_username(username) else {
        return Ok(None);
    };

    if !looks_like_kosync_userkey(userkey) {
        return Ok(None);
    }

    let Some(stored_user) =
        users::find_user_by_normalized_username(db, &normalized_username).await?
    else {
        return Ok(None);
    };
    let Some(userkey_hash) = stored_user.kosync_userkey_hash.as_deref() else {
        return Ok(None);
    };

    if !verify_kosync_userkey(userkey, userkey_hash)? {
        return Ok(None);
    }

    Ok(Some(stored_user.user))
}

fn header_value_str<'a>(value: Option<&'a HeaderValue>) -> Option<&'a str> {
    value.and_then(|value| value.to_str().ok())
}

fn looks_like_kosync_userkey(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::kosync_userkey;

    #[test]
    fn kosync_userkey_matches_md5_hex() {
        assert_eq!(
            kosync_userkey("password"),
            "5f4dcc3b5aa765d61d8327deb882cf99"
        );
    }
}
