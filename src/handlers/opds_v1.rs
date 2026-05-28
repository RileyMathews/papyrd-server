use askama::{Template, filters::urlencode};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth,
    domain::publication::{ContributorRole, OpdsPublicationSummary, PublicationDetail},
    error::AppError,
    repositories::publications::{self, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PublicationOrder},
    state::AppState,
};

const ACQUISITION_REL: &str = "http://opds-spec.org/acquisition";
const IMAGE_REL: &str = "http://opds-spec.org/image";
const THUMBNAIL_REL: &str = "http://opds-spec.org/image/thumbnail";
const SORT_NEW_REL: &str = "http://opds-spec.org/sort/new";
const NAVIGATION_FEED_MEDIA_TYPE: &str =
    "application/atom+xml;profile=opds-catalog;kind=navigation";
const ACQUISITION_FEED_MEDIA_TYPE: &str =
    "application/atom+xml;profile=opds-catalog;kind=acquisition";
const ENTRY_MEDIA_TYPE: &str = "application/atom+xml;type=entry;profile=opds-catalog";

#[derive(Deserialize)]
pub struct PaginationParams {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Template)]
#[template(path = "opds_v1/navigation.xml")]
struct NavigationFeedTemplate<'a> {
    id: String,
    title: &'a str,
    self_href: &'a str,
    updated: String,
    entries: &'a [NavigationEntry],
    navigation_feed_media_type: &'static str,
}

#[derive(Template)]
#[template(path = "opds_v1/acquisition.xml")]
struct AcquisitionFeedTemplate<'a> {
    id: String,
    title: &'a str,
    self_href: String,
    previous_href: Option<String>,
    next_href: Option<String>,
    updated: String,
    publications: Vec<PublicationEntry>,
    navigation_feed_media_type: &'static str,
    acquisition_feed_media_type: &'static str,
    entry_media_type: &'static str,
    acquisition_rel: &'static str,
    image_rel: &'static str,
    thumbnail_rel: &'static str,
}

#[derive(Template)]
#[template(path = "opds_v1/entry.xml")]
struct EntryTemplate {
    publication: PublicationEntry,
    entry_media_type: &'static str,
    acquisition_rel: &'static str,
    image_rel: &'static str,
    thumbnail_rel: &'static str,
}

pub async fn root(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let updated_at = Utc::now();
    let entries = vec![
        NavigationEntry::new(
            "All books",
            "/opdsv1/publications",
            ACQUISITION_FEED_MEDIA_TYPE,
            "subsection",
            "All books in this catalog.",
        ),
        NavigationEntry::new(
            "Recent books",
            "/opdsv1/publications/recent",
            ACQUISITION_FEED_MEDIA_TYPE,
            SORT_NEW_REL,
            "Recently added books in this catalog.",
        ),
        NavigationEntry::new(
            "Authors",
            "/opdsv1/authors",
            NAVIGATION_FEED_MEDIA_TYPE,
            "subsection",
            "Browse books by author.",
        ),
    ];

    let body = NavigationFeedTemplate {
        id: stable_id("/opdsv1"),
        title: "Papyrd catalog",
        self_href: "/opdsv1",
        updated: format_datetime(updated_at),
        entries: &entries,
        navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
    }
    .render()?;

    xml_response(body, NAVIGATION_FEED_MEDIA_TYPE)
}

pub async fn publications_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let (page, per_page) = pagination(params);
    let result =
        publications::list_opds_publications(state.db(), page, per_page, PublicationOrder::Title)
            .await?;

    let total_pages = if result.total_items == 0 {
        1
    } else {
        ((result.total_items - 1) / per_page) + 1
    };
    let updated_at = result
        .items
        .iter()
        .map(|publication| publication.updated_at)
        .max()
        .unwrap_or_else(Utc::now);
    let self_href = page_href("/opdsv1/publications", page, per_page);
    let body = AcquisitionFeedTemplate {
        id: stable_id(&self_href),
        title: "All books",
        self_href,
        previous_href: (page > 1).then(|| page_href("/opdsv1/publications", page - 1, per_page)),
        next_href: (page < total_pages)
            .then(|| page_href("/opdsv1/publications", page + 1, per_page)),
        updated: format_datetime(updated_at),
        publications: result
            .items
            .iter()
            .map(PublicationEntry::from_summary)
            .collect(),
        navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
        acquisition_feed_media_type: ACQUISITION_FEED_MEDIA_TYPE,
        entry_media_type: ENTRY_MEDIA_TYPE,
        acquisition_rel: ACQUISITION_REL,
        image_rel: IMAGE_REL,
        thumbnail_rel: THUMBNAIL_REL,
    }
    .render()?;

    xml_response(body, ACQUISITION_FEED_MEDIA_TYPE)
}

pub async fn recent_publications_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let (page, per_page) = pagination(params);
    let result =
        publications::list_opds_publications(state.db(), page, per_page, PublicationOrder::Recent)
            .await?;

    let total_pages = if result.total_items == 0 {
        1
    } else {
        ((result.total_items - 1) / per_page) + 1
    };
    let updated_at = result
        .items
        .iter()
        .map(|publication| publication.updated_at)
        .max()
        .unwrap_or_else(Utc::now);
    let self_href = page_href("/opdsv1/publications/recent", page, per_page);
    let body = AcquisitionFeedTemplate {
        id: stable_id(&self_href),
        title: "Recent books",
        self_href,
        previous_href: (page > 1)
            .then(|| page_href("/opdsv1/publications/recent", page - 1, per_page)),
        next_href: (page < total_pages)
            .then(|| page_href("/opdsv1/publications/recent", page + 1, per_page)),
        updated: format_datetime(updated_at),
        publications: result
            .items
            .iter()
            .map(PublicationEntry::from_summary)
            .collect(),
        navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
        acquisition_feed_media_type: ACQUISITION_FEED_MEDIA_TYPE,
        entry_media_type: ENTRY_MEDIA_TYPE,
        acquisition_rel: ACQUISITION_REL,
        image_rel: IMAGE_REL,
        thumbnail_rel: THUMBNAIL_REL,
    }
    .render()?;

    xml_response(body, ACQUISITION_FEED_MEDIA_TYPE)
}

pub async fn authors_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let authors = publications::list_authors(state.db()).await?;
    let updated_at = Utc::now();
    let entries = authors
        .iter()
        .map(|author| {
            let href = format!("/opdsv1/authors/{}", urlencode(&author.key).unwrap());
            let book_label = if author.publication_count == 1 {
                "book"
            } else {
                "books"
            };

            NavigationEntry::new(
                &author.name,
                &href,
                ACQUISITION_FEED_MEDIA_TYPE,
                "subsection",
                &format!("{} {book_label}.", author.publication_count),
            )
        })
        .collect::<Vec<_>>();
    let body = NavigationFeedTemplate {
        id: stable_id("/opdsv1/authors"),
        title: "Authors",
        self_href: "/opdsv1/authors",
        updated: format_datetime(updated_at),
        entries: &entries,
        navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
    }
    .render()?;

    xml_response(body, NAVIGATION_FEED_MEDIA_TYPE)
}

pub async fn author_publications_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(author_key): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let author_key = author_key.trim().to_lowercase();
    let Some(author_name) = publications::find_author_name(state.db(), &author_key).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let (page, per_page) = pagination(params);
    let result =
        publications::list_opds_publications_by_author(state.db(), &author_key, page, per_page)
            .await?;
    let base_path = format!("/opdsv1/authors/{}", urlencode(&author_key).unwrap());
    let total_pages = if result.total_items == 0 {
        1
    } else {
        ((result.total_items - 1) / per_page) + 1
    };
    let updated_at = result
        .items
        .iter()
        .map(|publication| publication.updated_at)
        .max()
        .unwrap_or_else(Utc::now);
    let self_href = page_href(&base_path, page, per_page);
    let title = format!("Books by {author_name}");
    let body = AcquisitionFeedTemplate {
        id: stable_id(&self_href),
        title: &title,
        self_href,
        previous_href: (page > 1).then(|| page_href(&base_path, page - 1, per_page)),
        next_href: (page < total_pages).then(|| page_href(&base_path, page + 1, per_page)),
        updated: format_datetime(updated_at),
        publications: result
            .items
            .iter()
            .map(PublicationEntry::from_summary)
            .collect(),
        navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
        acquisition_feed_media_type: ACQUISITION_FEED_MEDIA_TYPE,
        entry_media_type: ENTRY_MEDIA_TYPE,
        acquisition_rel: ACQUISITION_REL,
        image_rel: IMAGE_REL,
        thumbnail_rel: THUMBNAIL_REL,
    }
    .render()?;

    xml_response(body, ACQUISITION_FEED_MEDIA_TYPE)
}

pub async fn publication_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(publication_id): Path<Uuid>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let Some(publication) =
        publications::find_publication_by_id(state.db(), publication_id).await?
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let body = EntryTemplate {
        publication: PublicationEntry::from_detail(&publication),
        entry_media_type: ENTRY_MEDIA_TYPE,
        acquisition_rel: ACQUISITION_REL,
        image_rel: IMAGE_REL,
        thumbnail_rel: THUMBNAIL_REL,
    }
    .render()?;

    xml_response(body, ENTRY_MEDIA_TYPE)
}

fn pagination(params: PaginationParams) -> (i64, i64) {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params
        .per_page
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);

    (page, per_page)
}

fn xml_response(body: String, content_type: &str) -> Result<Response, AppError> {
    let content_type = HeaderValue::from_str(content_type)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    Ok(response)
}

async fn require_basic_auth(db: &sqlx::PgPool, headers: &HeaderMap) -> Result<(), AppError> {
    let user = auth::basic_auth_user(db, headers.get(header::AUTHORIZATION)).await?;

    if user.is_some() {
        Ok(())
    } else {
        Err(AppError::OpdsUnauthorized)
    }
}

fn page_href(base_path: &str, page: i64, per_page: i64) -> String {
    format!("{base_path}?page={page}&per_page={per_page}")
}

fn format_datetime(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn stable_id(value: &str) -> String {
    let mut fragment = String::new();

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            fragment.push(character.to_ascii_lowercase());
        } else if !fragment.ends_with(':') {
            fragment.push(':');
        }
    }

    let fragment = fragment.trim_matches(':');

    if fragment.is_empty() {
        "urn:papyrd:opdsv1:root".to_owned()
    } else {
        format!("urn:papyrd:opdsv1:{fragment}")
    }
}

fn author_names(authors: &[String]) -> Vec<String> {
    if authors.is_empty() {
        vec!["Unknown".to_owned()]
    } else {
        authors.to_owned()
    }
}

struct NavigationEntry {
    title: String,
    href: String,
    media_type: String,
    rel: String,
    content: String,
    id: String,
}

impl NavigationEntry {
    fn new(title: &str, href: &str, media_type: &str, rel: &str, content: &str) -> Self {
        Self {
            title: title.to_owned(),
            href: href.to_owned(),
            media_type: media_type.to_owned(),
            rel: rel.to_owned(),
            content: content.to_owned(),
            id: stable_id(href),
        }
    }
}

struct PublicationEntry {
    title: String,
    atom_id: String,
    source_identifier: String,
    updated: String,
    authors: Vec<String>,
    contributors: Vec<String>,
    alternate_href: Option<String>,
    alternate_title: String,
    self_href: Option<String>,
    cover_href: Option<String>,
    cover_media_type: String,
    acquisition_href: Option<String>,
}

impl PublicationEntry {
    fn from_summary(publication: &OpdsPublicationSummary) -> Self {
        Self {
            title: publication.title.clone(),
            atom_id: format!("urn:uuid:{}", publication.id),
            source_identifier: publication.source_identifier.clone(),
            updated: format_datetime(publication.updated_at),
            authors: author_names(&publication.authors),
            contributors: Vec::new(),
            alternate_href: Some(format!("/opdsv1/publications/{}", publication.id)),
            alternate_title: format!("Complete catalog entry for {}", publication.title),
            self_href: None,
            cover_href: publication
                .cover_image_path
                .as_ref()
                .map(|_| format!("/books/{}/cover", publication.id)),
            cover_media_type: "image/jpeg".to_owned(),
            acquisition_href: Some(format!("/books/{}/download", publication.id)),
        }
    }

    fn from_detail(publication: &PublicationDetail) -> Self {
        let authors = publication
            .contributors
            .iter()
            .filter(|contributor| contributor.role == ContributorRole::Author)
            .map(|contributor| contributor.name.clone())
            .collect::<Vec<_>>();
        let contributors = publication
            .contributors
            .iter()
            .filter(|contributor| contributor.role != ContributorRole::Author)
            .map(|contributor| contributor.name.clone())
            .collect();

        Self {
            title: publication.title.clone(),
            atom_id: format!("urn:uuid:{}", publication.id),
            source_identifier: publication.source_identifier.clone(),
            updated: format_datetime(publication.updated_at),
            authors: author_names(&authors),
            contributors,
            alternate_href: None,
            alternate_title: String::new(),
            self_href: Some(format!("/opdsv1/publications/{}", publication.id)),
            cover_href: publication
                .cover_image_path
                .as_ref()
                .map(|_| format!("/books/{}/cover", publication.id)),
            cover_media_type: publication
                .cover_image_media_type
                .clone()
                .unwrap_or_else(|| "image/jpeg".to_owned()),
            acquisition_href: publication
                .epub_path
                .as_ref()
                .map(|_| format!("/books/{}/download", publication.id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn publications_feed_is_atom_acquisition_feed() {
        let publication_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let publication = OpdsPublicationSummary {
            id: publication_id,
            source_identifier: "urn:isbn:9780000000000".to_owned(),
            title: "A & B".to_owned(),
            authors: vec!["Jane <Doe>".to_owned()],
            updated_at: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            cover_image_path: Some("covers/book.jpg".to_owned()),
        };
        let self_href = page_href("/opdsv1/publications", 1, 20);
        let xml = AcquisitionFeedTemplate {
            id: stable_id(&self_href),
            title: "All books",
            self_href,
            previous_href: None,
            next_href: Some(page_href("/opdsv1/publications", 2, 20)),
            updated: format_datetime(publication.updated_at),
            publications: vec![PublicationEntry::from_summary(&publication)],
            navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
            acquisition_feed_media_type: ACQUISITION_FEED_MEDIA_TYPE,
            entry_media_type: ENTRY_MEDIA_TYPE,
            acquisition_rel: ACQUISITION_REL,
            image_rel: IMAGE_REL,
            thumbnail_rel: THUMBNAIL_REL,
        }
        .render()
        .unwrap();

        assert!(xml.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
        assert!(
            xml.contains("type=\"application/atom+xml;profile=opds-catalog;kind=acquisition\"")
        );
        assert!(xml.contains("href=\"/opdsv1/publications?page=2&amp;per_page=20\""));
        assert!(xml.contains("<title>A &amp; B</title>"));
        assert!(xml.contains("<name>Jane &lt;Doe&gt;</name>"));
        assert!(xml.contains("rel=\"http://opds-spec.org/acquisition\""));
        assert!(xml.contains("href=\"/books/11111111-1111-1111-1111-111111111111/download\""));
    }

    #[test]
    fn navigation_attributes_are_escaped() {
        let entries = vec![NavigationEntry::new(
            "A \"Quoted\" Link",
            "/opdsv1/authors/a&b",
            ACQUISITION_FEED_MEDIA_TYPE,
            "subsection",
            "Books by A & B.",
        )];

        let xml = NavigationFeedTemplate {
            id: stable_id("/opdsv1/authors"),
            title: "Authors",
            self_href: "/opdsv1/authors",
            updated: format_datetime(Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap()),
            entries: &entries,
            navigation_feed_media_type: NAVIGATION_FEED_MEDIA_TYPE,
        }
        .render()
        .unwrap();

        assert!(xml.contains("href=\"/opdsv1/authors/a&amp;b\""));
        assert!(xml.contains("title=\"A &quot;Quoted&quot; Link\""));
        assert!(xml.contains("<content type=\"text\">Books by A &amp; B.</content>"));
    }
}
