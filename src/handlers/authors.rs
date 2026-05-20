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
    handlers::nav::NavView,
    repositories::{publications, user_permissions},
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/authors.html")]
struct AuthorsTemplate<'a> {
    authors: &'a [AuthorSummary],
    nav: NavView,
    active_nav: &'static str,
}

#[derive(Template)]
#[template(path = "pages/author_detail.html")]
struct AuthorDetailTemplate<'a> {
    author_name: &'a str,
    publications: &'a [PublicationSummary],
    nav: NavView,
    active_nav: &'static str,
}

pub async fn index(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let permissions = user_permissions::permission_set_for_user(state.db(), user.id).await?;
    let nav = NavView::from_permissions(&permissions);

    let authors = publications::list_authors(state.db()).await?;
    let html = AuthorsTemplate {
        authors: &authors,
        nav,
        active_nav: "authors",
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn show(
    State(state): State<AppState>,
    Path(author_key): Path<String>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let permissions = user_permissions::permission_set_for_user(state.db(), user.id).await?;
    let nav = NavView::from_permissions(&permissions);

    let author_key = author_key.trim().to_lowercase();
    let Some(author_name) = publications::find_author_name(state.db(), &author_key).await? else {
        return Ok(Redirect::to("/authors").into_response());
    };
    let author_publications =
        publications::list_publications_by_author(state.db(), &author_key).await?;

    let html = AuthorDetailTemplate {
        author_name: &author_name,
        publications: &author_publications,
        nav,
        active_nav: "authors",
    }
    .render()?;

    Ok(Html(html).into_response())
}
