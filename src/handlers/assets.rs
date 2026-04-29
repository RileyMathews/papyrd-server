use std::path::{Component, Path, PathBuf};

use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use uuid::Uuid;

use crate::{auth, error::AppError, repositories::publications, state::AppState};

pub async fn download_epub(
    State(state): State<AppState>,
    AxumPath(publication_id): AxumPath<Uuid>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = authenticated_user(state.db(), &jar, &headers).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let Some(publication) =
        publications::find_publication_by_id(state.db(), publication_id).await?
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let Some(epub_path) = publication.epub_path.as_deref() else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let Some(absolute_path) = resolve_media_path(state.media_root(), epub_path) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let bytes = match tokio::fs::read(&absolute_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %absolute_path.display(), error = ?error, "publication epub missing on disk");
            return Ok(StatusCode::NOT_FOUND.into_response());
        }
        Err(error) => return Err(error.into()),
    };

    let filename =
        epub_download_filename(&publication.title, publication.original_filename.as_deref());
    let disposition = content_disposition_header(&filename)?;

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/epub+zip"),
        )
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(bytes))
        .map_err(|error| std::io::Error::other(error.to_string()).into())
}

pub async fn cover_image(
    State(state): State<AppState>,
    AxumPath(publication_id): AxumPath<Uuid>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = authenticated_user(state.db(), &jar, &headers).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let Some(publication) =
        publications::find_publication_by_id(state.db(), publication_id).await?
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let (Some(cover_path), Some(media_type)) = (
        publication.cover_image_path.as_deref(),
        publication.cover_image_media_type.as_deref(),
    ) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let Some(absolute_path) = resolve_media_path(state.media_root(), cover_path) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let bytes = match tokio::fs::read(&absolute_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %absolute_path.display(), error = ?error, "publication cover missing on disk");
            return Ok(StatusCode::NOT_FOUND.into_response());
        }
        Err(error) => return Err(error.into()),
    };

    let content_type = HeaderValue::from_str(media_type)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .map_err(|error| std::io::Error::other(error.to_string()).into())
}

fn resolve_media_path(media_root: &Path, storage_path: &str) -> Option<PathBuf> {
    let relative_path = Path::new(storage_path);

    if relative_path.is_absolute() {
        return None;
    }

    if relative_path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }

    Some(media_root.join(relative_path))
}

fn epub_download_filename(title: &str, original_filename: Option<&str>) -> String {
    let filename = original_filename
        .map(sanitize_filename)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let slug = title
                .chars()
                .map(|character| match character {
                    'a'..='z' | 'A'..='Z' | '0'..='9' => character,
                    _ => '_',
                })
                .collect::<String>()
                .trim_matches('_')
                .to_owned();

            if slug.is_empty() {
                "book".to_owned()
            } else {
                slug
            }
        });

    if filename.to_lowercase().ends_with(".epub") {
        filename
    } else {
        format!("{filename}.epub")
    }
}

fn sanitize_filename(input: &str) -> String {
    input
        .trim()
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | ' ' => character,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches(['.', ' '])
        .to_owned()
}

fn content_disposition_header(filename: &str) -> Result<HeaderValue, AppError> {
    let escaped = filename.replace('\\', "_").replace('"', "_");

    HeaderValue::from_str(&format!("attachment; filename=\"{escaped}\""))
        .map_err(|error| std::io::Error::other(error.to_string()).into())
}

async fn authenticated_user(
    db: &sqlx::PgPool,
    jar: &PrivateCookieJar,
    headers: &HeaderMap,
) -> Result<Option<crate::domain::user::User>, AppError> {
    if let Some(user) = auth::current_user(db, jar).await? {
        return Ok(Some(user));
    }

    auth::basic_auth_user(db, headers.get(header::AUTHORIZATION)).await
}
