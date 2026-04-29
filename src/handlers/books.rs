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
    repositories::{publications, reading_progress},
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/books.html")]
struct BooksTemplate<'a> {
    publications: &'a [PublicationSummary],
}

#[derive(Template)]
#[template(path = "pages/book_detail.html")]
struct BookDetailTemplate<'a> {
    publication: &'a PublicationDetail,
    sync_status: SyncStatusView,
}

struct SyncStatusView {
    synced: bool,
    percentage: String,
    progress: String,
    device: String,
    updated_at: String,
}

pub async fn index(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let publications = publications::list_publications(state.db()).await?;
    let html = BooksTemplate {
        publications: &publications,
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
                progress: String::new(),
                device: String::new(),
                updated_at: String::new(),
            };
        };

        Self {
            synced: true,
            percentage: format!("{:.0}%", progress.percentage * 100.0),
            progress: progress.progress,
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
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    if let Some(asset_paths) = publications::delete_publication(state.db(), publication_id).await? {
        for asset_path in asset_paths {
            let absolute_path = state.media_root().join(asset_path);

            if let Err(error) = tokio::fs::remove_file(&absolute_path).await {
                if error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(path = %absolute_path.display(), error = ?error, "failed to remove publication asset");
                }
            }
        }
    }

    Ok(Redirect::to("/books").into_response())
}
