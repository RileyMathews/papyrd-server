use std::path::Path;

use uuid::Uuid;

use crate::{
    domain::publication::{NewPublication, NewPublicationAsset},
    epub,
    error::AppError,
    kosync_hash,
    repositories::publications,
    state::AppState,
};

#[derive(Clone, Debug)]
pub struct IngestedEpub {
    pub title: String,
}

#[derive(Debug)]
pub enum IngestError {
    Duplicate,
    Invalid(String),
    App(AppError),
}

pub async fn ingest_epub(
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
            partial_md5: Some(kosync_hash::partial_md5(bytes)),
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

async fn write_upload(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
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
