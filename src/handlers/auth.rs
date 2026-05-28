use askama::Template;
use axum::{
    Form,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use serde::Deserialize;
use sqlx::Error as SqlxError;

use crate::{
    auth,
    error::AppError,
    permissions::Permission,
    repositories::{signup_invites, user_permissions, users},
    state::AppState,
};

#[derive(Deserialize)]
pub struct AuthForm {
    username: String,
    password: String,
    #[serde(default)]
    invite: Option<String>,
}

#[derive(Deserialize)]
pub struct SignupQuery {
    invite: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/signup.html")]
struct SignupTemplate<'a> {
    username: &'a str,
    error: Option<&'a str>,
    signup_closed: bool,
    initial_setup: bool,
    invite_key: Option<&'a str>,
    invite_unlocked: bool,
    invalid_invite: bool,
}

struct SignupAvailability {
    signup_closed: bool,
    initial_setup: bool,
    invite_unlocked: bool,
}

#[derive(Template)]
#[template(path = "pages/signin.html")]
struct SigninTemplate<'a> {
    username: &'a str,
    error: Option<&'a str>,
}

pub async fn signup_form(
    State(state): State<AppState>,
    Query(query): Query<SignupQuery>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let invite_key = normalize_invite_key(query.invite.as_deref());
    let signup_availability = signup_availability(&state, invite_key.as_deref()).await?;
    let status = if signup_availability.signup_closed {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::OK
    };

    let html = SignupTemplate {
        username: "",
        error: None,
        signup_closed: signup_availability.signup_closed,
        initial_setup: signup_availability.initial_setup,
        invite_key: invite_key.as_deref(),
        invite_unlocked: signup_availability.invite_unlocked,
        invalid_invite: signup_availability.signup_closed && invite_key.is_some(),
    }
    .render()?;

    Ok((status, Html(html)).into_response())
}

pub async fn signup(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let invite_key = normalize_invite_key(form.invite.as_deref());
    let signup_availability = signup_availability(&state, invite_key.as_deref()).await?;
    if signup_availability.signup_closed {
        let html = SignupTemplate {
            username: "",
            error: None,
            signup_closed: true,
            initial_setup: signup_availability.initial_setup,
            invite_key: invite_key.as_deref(),
            invite_unlocked: false,
            invalid_invite: invite_key.is_some(),
        }
        .render()?;

        return Ok((StatusCode::FORBIDDEN, Html(html)).into_response());
    }

    let username = form.username.trim();
    let password = form.password.trim();
    let Some(normalized_username) = auth::normalize_username(username) else {
        let html = SignupTemplate {
            username,
            error: Some("Username is required."),
            signup_closed: false,
            initial_setup: signup_availability.initial_setup,
            invite_key: invite_key.as_deref(),
            invite_unlocked: signup_availability.invite_unlocked,
            invalid_invite: false,
        }
        .render()?;

        return Ok(Html(html).into_response());
    };

    if password.is_empty() {
        let html = SignupTemplate {
            username,
            error: Some("Password is required."),
            signup_closed: false,
            initial_setup: signup_availability.initial_setup,
            invite_key: invite_key.as_deref(),
            invite_unlocked: signup_availability.invite_unlocked,
            invalid_invite: false,
        }
        .render()?;

        return Ok(Html(html).into_response());
    }

    if users::find_user_by_normalized_username(state.db(), &normalized_username)
        .await?
        .is_some()
    {
        let html = SignupTemplate {
            username,
            error: Some("That username is already taken."),
            signup_closed: false,
            initial_setup: signup_availability.initial_setup,
            invite_key: invite_key.as_deref(),
            invite_unlocked: signup_availability.invite_unlocked,
            invalid_invite: false,
        }
        .render()?;

        return Ok(Html(html).into_response());
    }

    let password_hash = auth::hash_password(password)?;
    let kosync_userkey_hash = auth::hash_kosync_userkey(&auth::kosync_userkey(password))?;
    let created_result = if signup_availability.initial_setup {
        create_initial_setup_user(
            &state,
            username,
            &normalized_username,
            &password_hash,
            &kosync_userkey_hash,
        )
        .await
        .map(Some)
    } else if signup_availability.invite_unlocked {
        let Some(invite_key) = invite_key.as_deref() else {
            let html = SignupTemplate {
                username,
                error: None,
                signup_closed: true,
                initial_setup: false,
                invite_key: None,
                invite_unlocked: false,
                invalid_invite: false,
            }
            .render()?;

            return Ok((StatusCode::FORBIDDEN, Html(html)).into_response());
        };

        create_invited_user(
            &state,
            invite_key,
            username,
            &normalized_username,
            &password_hash,
            &kosync_userkey_hash,
        )
        .await
    } else {
        users::create_user(
            state.db(),
            username,
            &normalized_username,
            &password_hash,
            &kosync_userkey_hash,
        )
        .await
        .map(Some)
    };

    let created = match created_result {
        Ok(Some(created)) => created,
        Ok(None) => {
            let html = SignupTemplate {
                username,
                error: Some("This invite link is no longer available."),
                signup_closed: true,
                initial_setup: false,
                invite_key: invite_key.as_deref(),
                invite_unlocked: false,
                invalid_invite: true,
            }
            .render()?;

            return Ok((StatusCode::FORBIDDEN, Html(html)).into_response());
        }
        Err(error)
            if matches!(
                &error,
                SqlxError::Database(database_error) if database_error.is_unique_violation()
            ) =>
        {
            let html = SignupTemplate {
                username,
                error: Some("That username is already taken."),
                signup_closed: false,
                initial_setup: signup_availability.initial_setup,
                invite_key: invite_key.as_deref(),
                invite_unlocked: signup_availability.invite_unlocked,
                invalid_invite: false,
            }
            .render()?;

            return Ok(Html(html).into_response());
        }
        Err(error) => return Err(error.into()),
    };
    let jar = auth::sign_in_jar(jar, created.user.id, state.session_cookie_secure());

    Ok((jar, Redirect::to("/")).into_response())
}

pub async fn signin_form(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let html = SigninTemplate {
        username: "",
        error: None,
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn signin(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    let username = form.username.trim();
    let password = form.password.trim();
    let Some(normalized_username) = auth::normalize_username(username) else {
        let html = SigninTemplate {
            username,
            error: Some("Enter your username."),
        }
        .render()?;

        return Ok(Html(html).into_response());
    };

    if password.is_empty() {
        let html = SigninTemplate {
            username,
            error: Some("Enter your password."),
        }
        .render()?;

        return Ok(Html(html).into_response());
    }

    let Some(stored_user) =
        users::find_user_by_normalized_username(state.db(), &normalized_username).await?
    else {
        let html = SigninTemplate {
            username,
            error: Some("Invalid username or password."),
        }
        .render()?;

        return Ok(Html(html).into_response());
    };

    if !auth::verify_password(password, &stored_user.password_hash)? {
        let html = SigninTemplate {
            username,
            error: Some("Invalid username or password."),
        }
        .render()?;

        return Ok(Html(html).into_response());
    }

    let jar = auth::sign_in_jar(jar, stored_user.user.id, state.session_cookie_secure());
    Ok((jar, Redirect::to("/")).into_response())
}

pub async fn signout(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    let jar = auth::sign_out_jar(jar, state.session_cookie_secure());
    (jar, Redirect::to("/signin"))
}

async fn signup_availability(
    state: &AppState,
    invite_key: Option<&str>,
) -> Result<SignupAvailability, AppError> {
    let has_users = users::has_any_users(state.db()).await?;
    let signup_limited = state.disable_signup_after_first_user() && has_users;
    let invite_unlocked = if signup_limited {
        match invite_key {
            Some(invite_key) => signup_invites::is_open_invite_key(state.db(), invite_key).await?,
            None => false,
        }
    } else {
        false
    };

    Ok(SignupAvailability {
        signup_closed: signup_limited && !invite_unlocked,
        initial_setup: !has_users,
        invite_unlocked,
    })
}

async fn create_initial_setup_user(
    state: &AppState,
    username: &str,
    normalized_username: &str,
    password_hash: &str,
    kosync_userkey_hash: &str,
) -> Result<users::StoredUser, SqlxError> {
    let mut transaction = state.db().begin().await?;
    let created = users::create_user(
        &mut *transaction,
        username,
        normalized_username,
        password_hash,
        kosync_userkey_hash,
    )
    .await?;
    user_permissions::grant_permissions(
        &mut *transaction,
        created.user.id,
        &Permission::ALL,
        Some(created.user.id),
    )
    .await?;
    transaction.commit().await?;

    Ok(created)
}

async fn create_invited_user(
    state: &AppState,
    invite_key: &str,
    username: &str,
    normalized_username: &str,
    password_hash: &str,
    kosync_userkey_hash: &str,
) -> Result<Option<users::StoredUser>, SqlxError> {
    let mut transaction = state.db().begin().await?;
    let created = users::create_user(
        &mut *transaction,
        username,
        normalized_username,
        password_hash,
        kosync_userkey_hash,
    )
    .await?;

    if !signup_invites::claim_invite(&mut *transaction, invite_key, created.user.id).await? {
        transaction.rollback().await?;
        return Ok(None);
    }

    transaction.commit().await?;

    Ok(Some(created))
}

fn normalize_invite_key(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
