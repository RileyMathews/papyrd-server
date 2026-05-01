use std::path::PathBuf;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Multipart, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use uuid::Uuid;

use crate::{
    auth,
    domain::publication::{NewPublication, NewPublicationAsset},
    epub,
    error::AppError,
    kosync_hash,
    repositories::publications,
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/upload.html")]
struct UploadTemplate<'a> {
    error: Option<&'a str>,
    summary: Option<UploadSummary>,
    results: Vec<UploadResult>,
}

#[derive(Clone, Debug)]
struct UploadSummary {
    uploaded_count: usize,
    duplicate_count: usize,
    failed_count: usize,
}

#[derive(Clone, Debug)]
struct UploadResult {
    filename: String,
    title: Option<String>,
    status_label: &'static str,
    status_class: &'static str,
    message: String,
}

#[derive(Clone, Debug)]
struct IngestedEpub {
    title: String,
}

#[derive(Debug)]
enum IngestError {
    Duplicate,
    Invalid(String),
    App(AppError),
}

pub async fn upload_form(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    render_upload(None, None, Vec::new())
}

pub async fn upload(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let mut results = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| std::io::Error::other("failed to read multipart field"))?
    {
        if !matches!(field.name(), Some("epubs") | Some("epub")) {
            continue;
        }

        let original_filename = field.file_name().map(str::to_owned);
        let has_filename = original_filename
            .as_deref()
            .is_some_and(|filename| !filename.trim().is_empty());
        let display_filename = original_filename
            .as_deref()
            .filter(|_| has_filename)
            .unwrap_or("Unnamed EPUB")
            .to_owned();
        let bytes = field
            .bytes()
            .await
            .map_err(|_| std::io::Error::other("failed to read multipart body"))?;

        if !has_filename && bytes.is_empty() {
            continue;
        }

        results.push(
            upload_result_for_file(&state, display_filename, original_filename, bytes).await?,
        );
    }

    if results.is_empty() {
        return render_upload(
            Some("Choose one or more EPUB files to upload."),
            None,
            results,
        );
    }

    let summary = UploadSummary {
        uploaded_count: results
            .iter()
            .filter(|result| result.status_class == "success")
            .count(),
        duplicate_count: results
            .iter()
            .filter(|result| result.status_class == "duplicate")
            .count(),
        failed_count: results
            .iter()
            .filter(|result| result.status_class == "error")
            .count(),
    };

    render_upload(None, Some(summary), results)
}

async fn upload_result_for_file(
    state: &AppState,
    filename: String,
    original_filename: Option<String>,
    bytes: Bytes,
) -> Result<UploadResult, AppError> {
    if bytes.is_empty() {
        return Ok(UploadResult {
            filename,
            title: None,
            status_label: "Failed",
            status_class: "error",
            message: "The uploaded file was empty.".to_owned(),
        });
    }

    match ingest_epub(state, original_filename, &bytes).await {
        Ok(ingested) => Ok(UploadResult {
            filename,
            title: Some(ingested.title),
            status_label: "Uploaded",
            status_class: "success",
            message: "Added to the catalog.".to_owned(),
        }),
        Err(IngestError::Duplicate) => Ok(UploadResult {
            filename,
            title: None,
            status_label: "Duplicate",
            status_class: "duplicate",
            message: "Matches an existing book and was not uploaded again.".to_owned(),
        }),
        Err(IngestError::Invalid(message)) => Ok(UploadResult {
            filename,
            title: None,
            status_label: "Failed",
            status_class: "error",
            message,
        }),
        Err(IngestError::App(error)) => Err(error),
    }
}

async fn ingest_epub(
    state: &AppState,
    original_filename: Option<String>,
    bytes: &[u8],
) -> Result<IngestedEpub, IngestError> {
    let parsed =
        epub::parse_metadata(bytes).map_err(|error| IngestError::Invalid(error.to_string()))?;

    if publications::source_identifier_exists(state.db(), &parsed.source_identifier)
        .await
        .map_err(AppError::from)
        .map_err(IngestError::App)?
    {
        return Err(IngestError::Duplicate);
    }

    let title = parsed.title.clone();
    let publication_id = Uuid::new_v4();
    let asset_id = Uuid::new_v4();
    let relative_path = format!("epubs/{publication_id}.epub");
    let absolute_path = state.media_root_path_buf().join(&relative_path);
    let cover_file = parsed.cover_image.as_ref().map(|cover_image| {
        let cover_asset_id = Uuid::new_v4();
        let relative_path = format!("covers/{cover_asset_id}.{}", cover_image.file_extension);
        let absolute_path = state.media_root_path_buf().join(&relative_path);

        (cover_asset_id, relative_path, absolute_path)
    });

    write_upload(&absolute_path, bytes)
        .await
        .map_err(IngestError::App)?;

    if let (Some(cover_image), Some((_, _, cover_absolute_path))) =
        (&parsed.cover_image, &cover_file)
    {
        write_upload(cover_absolute_path, &cover_image.bytes)
            .await
            .map_err(IngestError::App)?;
    }

    let publication = NewPublication {
        id: publication_id,
        source_identifier: parsed.source_identifier,
        title: parsed.title,
        contributors: parsed.contributors,
        primary_asset: NewPublicationAsset {
            id: asset_id,
            storage_path: relative_path,
            media_type: "application/epub+zip".to_owned(),
            byte_size: bytes.len() as i64,
            partial_md5: Some(kosync_hash::partial_md5(&bytes)),
            original_filename,
        },
        cover_asset: match (&parsed.cover_image, &cover_file) {
            (Some(cover_image), Some((cover_asset_id, cover_relative_path, _))) => {
                Some(NewPublicationAsset {
                    id: *cover_asset_id,
                    storage_path: cover_relative_path.clone(),
                    media_type: cover_image.media_type.clone(),
                    byte_size: cover_image.bytes.len() as i64,
                    partial_md5: None,
                    original_filename: None,
                })
            }
            _ => None,
        },
    };

    if let Err(error) = publications::create_publication(state.db(), &publication).await {
        let _ = tokio::fs::remove_file(&absolute_path).await;
        if let Some((_, _, cover_absolute_path)) = &cover_file {
            let _ = tokio::fs::remove_file(cover_absolute_path).await;
        }

        if is_duplicate_source_identifier_error(&error) {
            return Err(IngestError::Duplicate);
        }

        return Err(IngestError::App(error.into()));
    }

    Ok(IngestedEpub { title })
}

async fn write_upload(path: &PathBuf, bytes: &[u8]) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, bytes).await?;
    Ok(())
}

fn is_duplicate_source_identifier_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            database_error.constraint() == Some("publications_source_identifier_key")
        }
        _ => false,
    }
}

fn render_upload(
    error: Option<&str>,
    summary: Option<UploadSummary>,
    results: Vec<UploadResult>,
) -> Result<Response, AppError> {
    let html = UploadTemplate {
        error,
        summary,
        results,
    }
    .render()?;
    Ok(Html(html).into_response())
}
