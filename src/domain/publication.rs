use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContributorRole {
    Author,
    Editor,
    Translator,
    Illustrator,
    Contributor,
}

impl ContributorRole {
    pub fn as_db_value(&self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Editor => "editor",
            Self::Translator => "translator",
            Self::Illustrator => "illustrator",
            Self::Contributor => "contributor",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "author" => Self::Author,
            "editor" => Self::Editor,
            "translator" => Self::Translator,
            "illustrator" => Self::Illustrator,
            _ => Self::Contributor,
        }
    }
}

impl std::fmt::Display for ContributorRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_value())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contributor {
    pub name: String,
    pub role: ContributorRole,
}

impl Contributor {
    pub fn role_suffix(&self) -> &'static str {
        match self.role {
            ContributorRole::Contributor => "",
            ContributorRole::Author => " (author)",
            ContributorRole::Editor => " (editor)",
            ContributorRole::Translator => " (translator)",
            ContributorRole::Illustrator => " (illustrator)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PublicationSummary {
    pub id: Uuid,
    pub source_identifier: String,
    pub title: String,
    pub contributors: String,
    pub cover_image_path: Option<String>,
    pub title_initial: String,
}

#[derive(Clone, Debug)]
pub struct AuthorSummary {
    pub key: String,
    pub name: String,
    pub publication_count: i64,
}

#[derive(Clone, Debug)]
pub struct PublicationDetail {
    pub id: Uuid,
    pub source_identifier: String,
    pub title: String,
    pub contributors: Vec<Contributor>,
    pub updated_at: DateTime<Utc>,
    pub cover_image_path: Option<String>,
    pub cover_image_media_type: Option<String>,
    pub title_initial: String,
    pub epub_path: Option<String>,
    pub epub_partial_md5: Option<String>,
    pub original_filename: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OpdsPublicationSummary {
    pub id: Uuid,
    pub source_identifier: String,
    pub title: String,
    pub authors: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub cover_image_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewPublication {
    pub id: Uuid,
    pub source_identifier: String,
    pub title: String,
    pub contributors: Vec<Contributor>,
    pub primary_asset: NewPublicationAsset,
    pub cover_asset: Option<NewPublicationAsset>,
}

#[derive(Clone, Debug)]
pub struct NewPublicationAsset {
    pub id: Uuid,
    pub storage_path: String,
    pub media_type: String,
    pub byte_size: i64,
    pub partial_md5: Option<String>,
    pub original_filename: Option<String>,
}
