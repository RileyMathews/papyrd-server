use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;

use crate::{
    auth,
    domain::publication::{AuthorSummary, PublicationSummary},
    error::AppError,
    repositories::publications,
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/authors.html")]
struct AuthorsTemplate<'a> {
    authors: &'a [AuthorSummary],
}

#[derive(Template)]
#[template(path = "pages/author_detail.html")]
struct AuthorDetailTemplate<'a> {
    author_name: &'a str,
    publications: &'a [PublicationSummary],
}

pub async fn index(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let authors = publications::list_authors(state.db()).await?;
    let html = AuthorsTemplate { authors: &authors }.render()?;

    Ok(Html(html).into_response())
}

pub async fn show(
    State(state): State<AppState>,
    Path(author_key): Path<String>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let author_key = author_key.trim().to_lowercase();
    let Some(author_name) = publications::find_author_name(state.db(), &author_key).await? else {
        return Ok(Redirect::to("/authors").into_response());
    };
    let author_publications =
        publications::list_publications_by_author(state.db(), &author_key).await?;

    let html = AuthorDetailTemplate {
        author_name: &author_name,
        publications: &author_publications,
    }
    .render()?;

    Ok(Html(html).into_response())
}
