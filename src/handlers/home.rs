use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;

use crate::{auth, error::AppError, state::AppState};

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate<'a> {
    username: &'a str,
}

pub async fn index(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let html = HomeTemplate {
        username: &user.username,
    }
    .render()?;
    Ok(Html(html).into_response())
}
