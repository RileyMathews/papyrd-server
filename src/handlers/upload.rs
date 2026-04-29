use std::path::PathBuf;

use askama::Template;
use axum::{
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
}

pub async fn upload_form(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    render_upload(None)
}

pub async fn upload(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let Some(_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| std::io::Error::other("failed to read multipart field"))?
    else {
        return render_upload(Some("Choose an EPUB file to upload."));
    };

    if field.name() != Some("epub") {
        return render_upload(Some("Choose an EPUB file to upload."));
    }

    let original_filename = field.file_name().map(str::to_owned);
    let bytes = field
        .bytes()
        .await
        .map_err(|_| std::io::Error::other("failed to read multipart body"))?;

    if bytes.is_empty() {
        return render_upload(Some("The uploaded file was empty."));
    }

    let parsed = match epub::parse_metadata(&bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            let message = error.to_string();
            return render_upload(Some(&message));
        }
    };

    if publications::source_identifier_exists(state.db(), &parsed.source_identifier).await? {
        return render_upload(Some(
            "That EPUB matches an existing book and was not uploaded again.",
        ));
    }

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

    write_upload(&absolute_path, &bytes).await?;

    if let (Some(cover_image), Some((_, _, cover_absolute_path))) =
        (&parsed.cover_image, &cover_file)
    {
        write_upload(cover_absolute_path, &cover_image.bytes).await?;
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
        return Err(error.into());
    }

    Ok(Redirect::to("/").into_response())
}

async fn write_upload(path: &PathBuf, bytes: &[u8]) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, bytes).await?;
    Ok(())
}

fn render_upload(error: Option<&str>) -> Result<Response, AppError> {
    let html = UploadTemplate { error }.render()?;
    Ok(Html(html).into_response())
}
