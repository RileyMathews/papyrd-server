use std::{ffi::OsStr, path::PathBuf};

use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use papyrd::{
    config::Config,
    ingest::{self, IngestError},
    state,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::from_env()?;
    let ingest_root = config.ingest_root.clone();
    tokio::fs::create_dir_all(&ingest_root).await?;
    tokio::fs::create_dir_all(ingest_root.join("failed")).await?;

    let state = state::AppState::new(config).await?;

    info!(path = %ingest_root.display(), "starting initial ingest directory scan");
    process_existing_files(&state, &ingest_root).await?;
    info!(path = %ingest_root.display(), "initial ingest directory scan completed");

    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<notify::Event, notify::Error>| match result {
            Ok(event) => {
                debug!(kind = ?event.kind, paths = ?event.paths, "watch event received");

                if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    debug!(kind = ?event.kind, "ignoring non create/modify watch event");
                    return;
                }

                for path in event.paths {
                    debug!(path = %path.display(), "queueing file from watch event");
                    let _ = tx.send(path);
                }
            }
            Err(error) => {
                error!(error = ?error, "file watch event error");
            }
        },
        NotifyConfig::default(),
    )?;

    watcher.watch(&ingest_root, RecursiveMode::NonRecursive)?;

    info!(path = %ingest_root.display(), "starting ingest watcher");

    while let Some(path) = rx.recv().await {
        if !is_epub(&path) {
            debug!(path = %path.display(), "ignoring non-epub path");
            continue;
        }

        debug!(path = %path.display(), "processing watched epub path");
        if let Err(error) = process_file(&state, &ingest_root, &path).await {
            error!(path = %path.display(), error = ?error, "failed to process ingest file");
        }
    }

    Ok(())
}

async fn process_existing_files(
    state: &state::AppState,
    ingest_root: &PathBuf,
) -> Result<(), std::io::Error> {
    let mut entries = tokio::fs::read_dir(ingest_root).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = entry.file_type().await?;
        if !file_type.is_file() || !is_epub(&path) {
            debug!(path = %path.display(), "ignoring existing non-epub path");
            continue;
        }

        debug!(path = %path.display(), "processing existing epub found during startup scan");

        if let Err(error) = process_file(state, ingest_root, &path).await {
            error!(path = %path.display(), error = ?error, "failed to process existing ingest file");
        }
    }

    Ok(())
}

async fn process_file(
    state: &state::AppState,
    ingest_root: &PathBuf,
    path: &PathBuf,
) -> Result<(), std::io::Error> {
    if !tokio::fs::try_exists(path).await? {
        debug!(path = %path.display(), "path no longer exists, skipping");
        return Ok(());
    }

    let metadata = tokio::fs::metadata(path).await?;
    let bytes = tokio::fs::read(path).await?;
    let has_zip_signature = bytes.starts_with(b"PK");
    debug!(
        path = %path.display(),
        byte_len = bytes.len(),
        metadata_len = metadata.len(),
        has_zip_signature,
        "read ingest candidate file"
    );
    let original_filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .map(ToOwned::to_owned);

    info!(path = %path.display(), "starting ingest");

    match ingest::ingest_epub(state, original_filename, &bytes).await {
        Ok(ingested) => {
            info!(path = %path.display(), title = %ingested.title, "ingest succeeded");
            let _ = tokio::fs::remove_file(path).await;
        }
        Err(IngestError::Duplicate) => {
            warn!(path = %path.display(), "duplicate ingest file, removing");
            let _ = tokio::fs::remove_file(path).await;
        }
        Err(IngestError::Invalid(message)) => {
            warn!(
                path = %path.display(),
                reason = %message,
                byte_len = bytes.len(),
                has_zip_signature,
                "invalid ingest file, moving to failed/"
            );
            let failed_path = ingest_root.join("failed").join(
                path.file_name()
                    .unwrap_or_else(|| OsStr::new("unknown.epub")),
            );
            if let Err(error) = tokio::fs::rename(path, &failed_path).await {
                warn!(
                    path = %path.display(),
                    failed_path = %failed_path.display(),
                    error = ?error,
                    "rename to failed path failed, deleting file instead"
                );
                let _ = tokio::fs::remove_file(path).await;
            } else {
                warn!(
                    from = %path.display(),
                    to = %failed_path.display(),
                    "moved invalid ingest file to failed directory"
                );
            }
        }
        Err(IngestError::App(error)) => {
            error!(path = %path.display(), error = ?error, "app ingest error, keeping file for retry");
        }
    }

    Ok(())
}

fn is_epub(path: &PathBuf) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("epub"))
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,papyrd=debug,tower_http=debug".into()),
        )
        .with_target(false)
        .compact()
        .init();
}
