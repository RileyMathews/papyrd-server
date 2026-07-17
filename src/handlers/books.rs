use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use uuid::Uuid;

use crate::{
    auth,
    domain::publication::{PublicationDetail, PublicationSummary},
    error::AppError,
    handlers::nav::NavView,
    permissions::Permission,
    repositories::{publications, reading_progress, user_permissions},
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/books.html")]
struct BooksTemplate<'a> {
    publications: &'a [PublicationSummary],
    nav: NavView,
    active_nav: &'static str,
}

#[derive(Template)]
#[template(path = "pages/book_detail.html")]
struct BookDetailTemplate<'a> {
    publication: &'a PublicationDetail,
    sync_status: SyncStatusView,
    nav: NavView,
    active_nav: &'static str,
}

struct SyncStatusView {
    synced: bool,
    percentage: String,
    percentage_value: f64,
    device: String,
    updated_at: String,
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

    let publications = publications::list_publications(state.db()).await?;
    let html = BooksTemplate {
        publications: &publications,
        nav,
        active_nav: "books",
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn show(
    State(state): State<AppState>,
    Path(publication_id): Path<Uuid>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let permissions = user_permissions::permission_set_for_user(state.db(), user.id).await?;
    let nav = NavView::from_permissions(&permissions);

    let Some(publication) =
        publications::find_publication_by_id(state.db(), publication_id).await?
    else {
        return Ok(Redirect::to("/books").into_response());
    };

    let progress = match kosync_document_for_publication(&publication) {
        Some(document) => {
            reading_progress::find_by_user_and_document(state.db(), user.id, &document).await?
        }
        None => None,
    };

    let html = BookDetailTemplate {
        publication: &publication,
        sync_status: SyncStatusView::from_progress(progress),
        nav,
        active_nav: "books",
    }
    .render()?;

    Ok(Html(html).into_response())
}

impl SyncStatusView {
    fn from_progress(progress: Option<crate::domain::reading_progress::ReadingProgress>) -> Self {
        let Some(progress) = progress else {
            return Self {
                synced: false,
                percentage: String::new(),
                percentage_value: 0.0,
                device: String::new(),
                updated_at: String::new(),
            };
        };

        let pct_display = progress.percentage.clamp(0.0, 1.0);
        Self {
            synced: true,
            percentage: format!("{:.0}%", pct_display * 100.0),
            percentage_value: pct_display * 100.0,
            device: progress.device,
            updated_at: progress.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        }
    }
}

fn kosync_document_for_publication(publication: &PublicationDetail) -> Option<String> {
    publication
        .epub_partial_md5
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(publication_id): Path<Uuid>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let permissions = user_permissions::permission_set_for_user(state.db(), user.id).await?;
    permissions.require(Permission::PublicationDelete)?;

    if let Some(asset_paths) = publications::delete_publication(state.db(), publication_id).await? {
        for asset_path in asset_paths {
            let absolute_path = state.media_root().join(asset_path);

            if let Err(error) = tokio::fs::remove_file(&absolute_path).await
                && error.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(path = %absolute_path.display(), error = ?error, "failed to remove publication asset");
            }
        }
    }

    Ok(Redirect::to("/books").into_response())
}
