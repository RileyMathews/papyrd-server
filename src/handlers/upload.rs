use askama::Template;
use axum::{
    body::Bytes,
    extract::{Multipart, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;

use crate::{
    auth,
    error::AppError,
    ingest::{self, IngestError},
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

    match ingest::ingest_epub(state, original_filename, &bytes).await {
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
