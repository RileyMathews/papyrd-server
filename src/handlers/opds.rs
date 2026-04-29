use askama::filters::urlencode;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth,
    domain::publication::{ContributorRole, OpdsPublicationSummary, PublicationDetail},
    error::AppError,
    repositories::publications::{self, DEFAULT_PAGE_SIZE, PaginatedResult, PublicationOrder},
    state::AppState,
};

const OPDS_FEED_MEDIA_TYPE: &str = "application/opds+json";
const OPDS_PUBLICATION_MEDIA_TYPE: &str = "application/opds-publication+json";
#[derive(Deserialize)]
pub struct PaginationParams {
    page: Option<i64>,
    per_page: Option<i64>,
}

pub async fn root(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let feed = OpdsFeed {
        metadata: FeedMetadata {
            title: "Papyrd catalog".to_owned(),
            number_of_items: None,
        },
        links: vec![self_link("/opds")],
        navigation: Some(vec![
            NavLink::new("/opds/publications", "All books"),
            NavLink::new("/opds/publications/recent", "Recent books"),
            NavLink::new("/opds/authors", "Authors"),
        ]),
        publications: None,
    };

    opds_json_response(&feed, OPDS_FEED_MEDIA_TYPE)
}

pub async fn publications_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(DEFAULT_PAGE_SIZE);
    let result =
        publications::list_opds_publications(state.db(), page, per_page, PublicationOrder::Title)
            .await?;

    render_publications_feed("All books", "/opds/publications", page, per_page, result)
}

pub async fn recent_publications_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(DEFAULT_PAGE_SIZE);
    let result =
        publications::list_opds_publications(state.db(), page, per_page, PublicationOrder::Recent)
            .await?;

    render_publications_feed(
        "Recent books",
        "/opds/publications/recent",
        page,
        per_page,
        result,
    )
}

pub async fn authors_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    require_basic_auth(state.db(), &headers).await?;

    let authors = publications::list_authors(state.db()).await?;
    let navigation = authors
        .into_iter()
        .map(|author| {
            NavLink::new(
                &format!("/opds/authors/{}", urlencode(&author.key).unwrap()),
                &author.name,
            )
        })
        .collect();

    let feed = OpdsFeed {
        metadata: FeedMetadata {
            title: "Authors".to_owned(),
            number_of_items: None,
        },
        links: vec![self_link("/opds/authors")],
        navigation: Some(navigation),
        publications: None,
    };

    opds_json_response(&feed, OPDS_FEED_MEDIA_TYPE)
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

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(DEFAULT_PAGE_SIZE);
    let result =
        publications::list_opds_publications_by_author(state.db(), &author_key, page, per_page)
            .await?;

    render_publications_feed(
        &format!("Books by {author_name}"),
        &format!("/opds/authors/{}", urlencode(&author_key).unwrap()),
        page,
        per_page,
        result,
    )
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

    let document = publication_document_body(&publication);
    opds_json_response(&document, OPDS_PUBLICATION_MEDIA_TYPE)
}

fn render_publications_feed(
    title: &str,
    base_path: &str,
    page: i64,
    per_page: i64,
    result: PaginatedResult<OpdsPublicationSummary>,
) -> Result<Response, AppError> {
    let page = page.max(1);
    let total_pages = if result.total_items == 0 {
        1
    } else {
        ((result.total_items - 1) / per_page.max(1)) + 1
    };

    let mut links = vec![self_link(&page_href(base_path, page, per_page))];

    if page > 1 {
        links.push(OpdsLink::new(
            &page_href(base_path, page - 1, per_page),
            OPDS_FEED_MEDIA_TYPE,
            Some("previous"),
            None,
        ));
    }

    if page < total_pages {
        links.push(OpdsLink::new(
            &page_href(base_path, page + 1, per_page),
            OPDS_FEED_MEDIA_TYPE,
            Some("next"),
            None,
        ));
    }

    let publications = result.items.iter().map(publication_feed_entry).collect();
    let feed = OpdsFeed {
        metadata: FeedMetadata {
            title: title.to_owned(),
            number_of_items: Some(result.total_items),
        },
        links,
        navigation: None,
        publications: Some(publications),
    };

    opds_json_response(&feed, OPDS_FEED_MEDIA_TYPE)
}

fn publication_feed_entry(publication: &OpdsPublicationSummary) -> OpdsPublication {
    let mut links = vec![OpdsLink::new(
        &format!("/opds/publications/{}", publication.id),
        OPDS_PUBLICATION_MEDIA_TYPE,
        Some("self"),
        None,
    )];

    links.push(OpdsLink::new(
        &format!("/books/{}/download", publication.id),
        "application/epub+zip",
        Some("http://opds-spec.org/acquisition"),
        Some("Download EPUB"),
    ));

    OpdsPublication {
        metadata: publication_metadata(
            &publication.source_identifier,
            &publication.title,
            &publication.authors,
            publication.updated_at,
        ),
        links,
        images: publication.cover_image_path.as_ref().map(|_| {
            vec![OpdsLink::new(
                &format!("/books/{}/cover", publication.id),
                "image/jpeg",
                None,
                Some("Cover image"),
            )]
        }),
    }
}

fn publication_document_body(publication: &PublicationDetail) -> OpdsPublication {
    let authors = publication
        .contributors
        .iter()
        .filter(|contributor| contributor.role == ContributorRole::Author)
        .map(|contributor| contributor.name.clone())
        .collect::<Vec<_>>();

    let mut links = vec![OpdsLink::new(
        &format!("/opds/publications/{}", publication.id),
        OPDS_PUBLICATION_MEDIA_TYPE,
        Some("self"),
        None,
    )];

    if publication.epub_path.is_some() {
        links.push(OpdsLink::new(
            &format!("/books/{}/download", publication.id),
            "application/epub+zip",
            Some("http://opds-spec.org/acquisition"),
            Some("Download EPUB"),
        ));
    }

    OpdsPublication {
        metadata: publication_metadata(
            &publication.source_identifier,
            &publication.title,
            &authors,
            publication.updated_at,
        ),
        links,
        images: publication.cover_image_path.as_ref().map(|_| {
            vec![OpdsLink::new(
                &format!("/books/{}/cover", publication.id),
                publication
                    .cover_image_media_type
                    .as_deref()
                    .unwrap_or("image/jpeg"),
                None,
                Some("Cover image"),
            )]
        }),
    }
}

fn publication_metadata(
    identifier: &str,
    title: &str,
    authors: &[String],
    updated_at: chrono::DateTime<Utc>,
) -> PublicationMetadata {
    PublicationMetadata {
        kind: "http://schema.org/EBook".to_owned(),
        identifier: identifier.to_owned(),
        title: title.to_owned(),
        modified: updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        author: if authors.is_empty() {
            None
        } else {
            Some(
                authors
                    .iter()
                    .map(|author| ContributorName {
                        name: author.clone(),
                    })
                    .collect(),
            )
        },
    }
}

fn self_link(href: &str) -> OpdsLink {
    OpdsLink::new(href, OPDS_FEED_MEDIA_TYPE, Some("self"), None)
}

fn page_href(base_path: &str, page: i64, per_page: i64) -> String {
    format!("{base_path}?page={page}&per_page={per_page}")
}

fn opds_json_response<T: Serialize>(value: &T, content_type: &str) -> Result<Response, AppError> {
    let body =
        serde_json::to_vec(value).map_err(|error| std::io::Error::other(error.to_string()))?;
    let content_type = HeaderValue::from_str(content_type)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut response = Response::new(axum::body::Body::from(body));
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
        Err(unauthorized_error())
    }
}

fn unauthorized_error() -> AppError {
    AppError::OpdsUnauthorized
}

#[derive(Serialize)]
struct OpdsFeed {
    metadata: FeedMetadata,
    links: Vec<OpdsLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    navigation: Option<Vec<NavLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publications: Option<Vec<OpdsPublication>>,
}

#[derive(Serialize)]
struct FeedMetadata {
    title: String,
    #[serde(rename = "numberOfItems", skip_serializing_if = "Option::is_none")]
    number_of_items: Option<i64>,
}

#[derive(Serialize)]
struct NavLink {
    href: String,
    title: String,
    #[serde(rename = "type")]
    media_type: String,
    rel: String,
}

impl NavLink {
    fn new(href: &str, title: &str) -> Self {
        Self {
            href: href.to_owned(),
            title: title.to_owned(),
            media_type: OPDS_FEED_MEDIA_TYPE.to_owned(),
            rel: "subsection".to_owned(),
        }
    }
}

#[derive(Serialize)]
struct OpdsPublication {
    metadata: PublicationMetadata,
    links: Vec<OpdsLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<OpdsLink>>,
}

#[derive(Serialize)]
struct PublicationMetadata {
    #[serde(rename = "@type")]
    kind: String,
    identifier: String,
    title: String,
    modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<Vec<ContributorName>>,
}

#[derive(Serialize)]
struct ContributorName {
    name: String,
}

#[derive(Serialize)]
struct OpdsLink {
    href: String,
    #[serde(rename = "type")]
    media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

impl OpdsLink {
    fn new(href: &str, media_type: &str, rel: Option<&str>, title: Option<&str>) -> Self {
        Self {
            href: href.to_owned(),
            media_type: media_type.to_owned(),
            rel: rel.map(str::to_owned),
            title: title.map(str::to_owned),
        }
    }
}
