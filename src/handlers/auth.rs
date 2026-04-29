use askama::Template;
use axum::{
    Form,
    extract::State,
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

    render_signup("", None)
}

pub async fn signup(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<AuthForm>,
) -> Result<Response, AppError> {
    if auth::current_user(state.db(), &jar).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let username = form.username.trim();
    let password = form.password.trim();
    let Some(normalized_username) = auth::normalize_username(username) else {
        return render_signup(username, Some("Username is required."));
    };

    if password.is_empty() {
        return render_signup(username, Some("Password is required."));
    }

    if users::find_user_by_normalized_username(state.db(), &normalized_username)
        .await?
        .is_some()
    {
        return render_signup(username, Some("That username is already taken."));
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
            return render_signup(username, Some("That username is already taken."));
        }
        Err(error) => return Err(error.into()),
    };
    let jar = auth::sign_in_jar(jar, created.user.id);

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

    let jar = auth::sign_in_jar(jar, stored_user.user.id);
    Ok((jar, Redirect::to("/")).into_response())
}

pub async fn signout(jar: PrivateCookieJar) -> impl IntoResponse {
    let jar = auth::sign_out_jar(jar);
    (jar, Redirect::to("/signin"))
}

fn render_signup(username: &str, error: Option<&str>) -> Result<Response, AppError> {
    let html = SignupTemplate { username, error }.render()?;
    Ok(Html(html).into_response())
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
