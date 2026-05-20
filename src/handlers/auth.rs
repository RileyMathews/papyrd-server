use askama::Template;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use serde::Deserialize;
use sqlx::Error as SqlxError;

use crate::{auth, error::AppError, repositories::users, state::AppState};

#[derive(Deserialize)]
pub struct AuthForm {
    username: String,
    password: String,
}

#[derive(Template)]
#[template(path = "pages/signup.html")]
struct SignupTemplate<'a> {
    username: &'a str,
    error: Option<&'a str>,
    signup_closed: bool,
    initial_setup: bool,
}

struct SignupAvailability {
    signup_closed: bool,
    initial_setup: bool,
}

#[derive(Template)]
#[template(path = "pages/signin.html")]
struct SigninTemplate<'a> {
    username: &'a str,
    error: Option<&'a str>,
}

pub async fn signup_form(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let signup_availability = signup_availability(&state).await?;
    let html = SignupTemplate {
        username: "",
        error: None,
        signup_closed: signup_availability.signup_closed,
        initial_setup: signup_availability.initial_setup,
    }
    .render()?;

    if signup_availability.signup_closed {
        return Ok((StatusCode::FORBIDDEN, Html(html)).into_response());
    }

    Ok(Html(html).into_response())
}

pub async fn signup(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let signup_availability = signup_availability(&state).await?;
    if signup_availability.signup_closed {
        let html = SignupTemplate {
            username: "",
            error: None,
            signup_closed: true,
            initial_setup: signup_availability.initial_setup,
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
        }
        .render()?;
        return Ok(Html(html).into_response());
    }

    let password_hash = auth::hash_password(password)?;
    let kosync_userkey_hash = auth::hash_kosync_userkey(&auth::kosync_userkey(password))?;
    let created = match users::create_user(
        state.db(),
        username,
        &normalized_username,
        &password_hash,
        &kosync_userkey_hash,
    )
    .await
    {
        Ok(created) => created,
        Err(error) if is_unique_violation(&error) => {
            let html = SignupTemplate {
                username,
                error: Some("That username is already taken."),
                signup_closed: false,
                initial_setup: signup_availability.initial_setup,
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

    render_signin("", None)
}

pub async fn signin(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    let username = form.username.trim();
    let password = form.password.trim();
    let Some(normalized_username) = auth::normalize_username(username) else {
        return render_signin(username, Some("Enter your username."));
    };

    if password.is_empty() {
        return render_signin(username, Some("Enter your password."));
    }

    let Some(stored_user) =
        users::find_user_by_normalized_username(state.db(), &normalized_username).await?
    else {
        return render_signin(username, Some("Invalid username or password."));
    };

    if !auth::verify_password(password, &stored_user.password_hash)? {
        return render_signin(username, Some("Invalid username or password."));
    }

    let jar = auth::sign_in_jar(jar, stored_user.user.id, state.session_cookie_secure());
    Ok((jar, Redirect::to("/")).into_response())
}

pub async fn signout(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    let jar = auth::sign_out_jar(jar, state.session_cookie_secure());
    (jar, Redirect::to("/signin"))
}

async fn signup_availability(state: &AppState) -> Result<SignupAvailability, AppError> {
    let has_users = users::has_any_users(state.db()).await?;

    Ok(SignupAvailability {
        signup_closed: state.disable_signup_after_first_user() && has_users,
        initial_setup: !has_users,
    })
}

fn render_signin(username: &str, error: Option<&str>) -> Result<Response, AppError> {
    let html = SigninTemplate { username, error }.render()?;
    Ok(Html(html).into_response())
}

fn is_unique_violation(error: &SqlxError) -> bool {
    match error {
        SqlxError::Database(database_error) => database_error.is_unique_violation(),
        _ => false,
    }
}
