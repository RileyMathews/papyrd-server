use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: Config,
    db: PgPool,
    media_root: PathBuf,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self, sqlx::Error> {
        let media_root = config.storage_root.join("media");

        std::fs::create_dir_all(&media_root)?;

        let db = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&db).await?;

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                db,
                media_root,
            }),
        })
    }

    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }

    pub fn session_key(&self) -> Key {
        self.inner.config.session_key.clone()
    }

    pub fn media_root(&self) -> &Path {
        self.inner.media_root.as_path()
    }

    pub fn media_root_path_buf(&self) -> PathBuf {
        self.inner.media_root.clone()
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(input: &AppState) -> Self {
        input.db().clone()
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        input.session_key()
    }
}
