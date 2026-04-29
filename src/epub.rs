use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

use quick_xml::{Reader, events::Event, name::QName};
use thiserror::Error;
use zip::ZipArchive;

use crate::domain::publication::{Contributor, ContributorRole};

#[derive(Debug)]
pub struct ParsedEpub {
    pub source_identifier: String,
    pub title: String,
    pub contributors: Vec<Contributor>,
    pub cover_image: Option<ParsedCoverImage>,
}

#[derive(Debug)]
pub struct ParsedCoverImage {
    pub file_extension: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

pub fn parse_metadata(bytes: &[u8]) -> Result<ParsedEpub, EpubError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|_| EpubError::InvalidArchive)?;
    let package_path = container_package_path(&mut archive)?;
    let package_document = read_zip_entry(&mut archive, &package_path)?;

    parse_package_document(&mut archive, &package_path, &package_document)
}

#[derive(Debug, Error)]
pub enum EpubError {
    #[error("The uploaded file is not a valid EPUB archive.")]
    InvalidArchive,
    #[error("The EPUB is missing package metadata.")]
    MissingPackageDocument,
    #[error("The EPUB package metadata could not be read.")]
    InvalidPackageDocument,
    #[error("The EPUB is missing a title.")]
    MissingTitle,
    #[error("The EPUB is missing an identifier.")]
    MissingIdentifier,
}

#[derive(Debug)]
struct PackageContributor {
    id: Option<String>,
    name: String,
    default_role: ContributorRole,
}

fn container_package_path<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String, EpubError> {
    let container_document = read_zip_entry(archive, "META-INF/container.xml")?;
    let mut reader = Reader::from_str(&container_document);

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref event)) | Ok(Event::Start(ref event))
                if event.name() == QName(b"rootfile") =>
            {
                for attribute in event.attributes().flatten() {
                    if attribute.key == QName(b"full-path") {
                        let value = attribute
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|_| EpubError::InvalidPackageDocument)?;
                        return Ok(value.into_owned());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(EpubError::InvalidPackageDocument),
            _ => {}
        }
    }

    Err(EpubError::MissingPackageDocument)
}

fn parse_package_document<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    package_path: &str,
    document: &str,
) -> Result<ParsedEpub, EpubError> {
    let mut reader = Reader::from_str(document);
    let mut title = None;
    let mut identifier = None;
    let mut contributors = Vec::new();
    let mut contributor_roles = HashMap::new();
    let mut cover_item_id = None;
    let mut cover_href = None;
    let mut cover_media_type = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref event)) if is_named(event.name(), b"title") => {
                let text = reader
                    .read_text(event.name())
                    .map_err(|_| EpubError::InvalidPackageDocument)?;
                if !text.trim().is_empty() && title.is_none() {
                    title = Some(text.trim().to_owned());
                }
            }
            Ok(Event::Start(ref event)) if is_named(event.name(), b"identifier") => {
                let text = reader
                    .read_text(event.name())
                    .map_err(|_| EpubError::InvalidPackageDocument)?;
                if !text.trim().is_empty() && identifier.is_none() {
                    identifier = Some(text.trim().to_owned());
                }
            }
            Ok(Event::Start(ref event)) if is_named(event.name(), b"creator") => {
                let contributor_id = attribute_value(event, reader.decoder(), b"id")?;
                let text = reader
                    .read_text(event.name())
                    .map_err(|_| EpubError::InvalidPackageDocument)?;
                let contributor_name = text.trim();

                if !contributor_name.is_empty() {
                    contributors.push(PackageContributor {
                        id: contributor_id,
                        name: contributor_name.to_owned(),
                        default_role: ContributorRole::Author,
                    });
                }
            }
            Ok(Event::Start(ref event)) if is_named(event.name(), b"contributor") => {
                let contributor_id = attribute_value(event, reader.decoder(), b"id")?;
                let text = reader
                    .read_text(event.name())
                    .map_err(|_| EpubError::InvalidPackageDocument)?;
                let contributor_name = text.trim();

                if !contributor_name.is_empty() {
                    contributors.push(PackageContributor {
                        id: contributor_id,
                        name: contributor_name.to_owned(),
                        default_role: ContributorRole::Contributor,
                    });
                }
            }
            Ok(Event::Empty(ref event)) if is_named(event.name(), b"meta") => {
                let mut name = None;
                let mut content = None;
                let mut property = None;
                let mut refines = None;

                for attribute in event.attributes().flatten() {
                    if is_named(attribute.key, b"name") {
                        name = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }

                    if is_named(attribute.key, b"content") {
                        content = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }

                    if is_named(attribute.key, b"property") {
                        property = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }

                    if is_named(attribute.key, b"refines") {
                        refines = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }
                }

                if name.as_deref() == Some("cover") {
                    cover_item_id = content.clone();
                }

                add_contributor_role_refinement(
                    &mut contributor_roles,
                    property.as_deref(),
                    refines.as_deref(),
                    content.as_deref(),
                );
            }
            Ok(Event::Start(ref event)) if is_named(event.name(), b"meta") => {
                let mut property = None;
                let mut refines = None;

                for attribute in event.attributes().flatten() {
                    if is_named(attribute.key, b"property") {
                        property = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }

                    if is_named(attribute.key, b"refines") {
                        refines = Some(
                            attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(|_| EpubError::InvalidPackageDocument)?
                                .into_owned(),
                        );
                    }
                }

                let value = reader
                    .read_text(event.name())
                    .map_err(|_| EpubError::InvalidPackageDocument)?;

                add_contributor_role_refinement(
                    &mut contributor_roles,
                    property.as_deref(),
                    refines.as_deref(),
                    Some(value.trim()),
                );
            }
            Ok(Event::Empty(ref event)) | Ok(Event::Start(ref event))
                if is_named(event.name(), b"item") =>
            {
                let mut item_id = None;
                let mut href = None;
                let mut media_type = None;
                let mut properties = None;

                for attribute in event.attributes().flatten() {
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|_| EpubError::InvalidPackageDocument)?
                        .into_owned();

                    if is_named(attribute.key, b"id") {
                        item_id = Some(value.clone());
                    }

                    if is_named(attribute.key, b"href") {
                        href = Some(value.clone());
                    }

                    if is_named(attribute.key, b"media-type") {
                        media_type = Some(value.clone());
                    }

                    if is_named(attribute.key, b"properties") {
                        properties = Some(value);
                    }
                }

                let is_cover_image = properties
                    .as_deref()
                    .map(|value| value.split_whitespace().any(|item| item == "cover-image"))
                    .unwrap_or(false);
                let matches_named_cover =
                    item_id.is_some() && cover_item_id.as_deref() == item_id.as_deref();

                if (is_cover_image || matches_named_cover)
                    && cover_href.is_none()
                    && href.is_some()
                    && media_type.is_some()
                {
                    cover_href = href;
                    cover_media_type = media_type;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(EpubError::InvalidPackageDocument),
            _ => {}
        }
    }

    let title = title.ok_or(EpubError::MissingTitle)?;
    let source_identifier = identifier.ok_or(EpubError::MissingIdentifier)?;
    let contributors = contributors
        .into_iter()
        .map(|contributor| Contributor {
            name: contributor.name,
            role: contributor
                .id
                .as_ref()
                .and_then(|id| contributor_roles.get(id))
                .cloned()
                .unwrap_or(contributor.default_role),
        })
        .collect();

    let cover_image = match (cover_href, cover_media_type) {
        (Some(href), Some(media_type)) => {
            let asset_path = resolve_archive_path(package_path, &href);
            let image_bytes = read_zip_binary_entry(archive, &asset_path).ok();

            image_bytes.and_then(|bytes| {
                cover_extension(&media_type, &href).map(|file_extension| ParsedCoverImage {
                    file_extension,
                    media_type,
                    bytes,
                })
            })
        }
        _ => None,
    };

    Ok(ParsedEpub {
        source_identifier,
        title,
        contributors,
        cover_image,
    })
}

fn attribute_value(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    attribute_name: &[u8],
) -> Result<Option<String>, EpubError> {
    for attribute in event.attributes().flatten() {
        if is_named(attribute.key, attribute_name) {
            return Ok(Some(
                attribute
                    .decode_and_unescape_value(decoder)
                    .map_err(|_| EpubError::InvalidPackageDocument)?
                    .into_owned(),
            ));
        }
    }

    Ok(None)
}

fn role_from_value(value: &str) -> ContributorRole {
    match value.trim().to_ascii_lowercase().as_str() {
        "aut" | "author" => ContributorRole::Author,
        "edt" | "editor" => ContributorRole::Editor,
        "trl" | "translator" => ContributorRole::Translator,
        "ill" | "illustrator" => ContributorRole::Illustrator,
        _ => ContributorRole::Contributor,
    }
}

fn add_contributor_role_refinement(
    contributor_roles: &mut HashMap<String, ContributorRole>,
    property: Option<&str>,
    refines: Option<&str>,
    value: Option<&str>,
) {
    if property != Some("role") {
        return;
    }

    let Some(refines) = refines else {
        return;
    };

    let Some(value) = value.map(str::trim) else {
        return;
    };

    if value.is_empty() {
        return;
    }

    contributor_roles.insert(
        refines.trim_start_matches('#').to_owned(),
        role_from_value(value),
    );
}

fn is_named(name: QName<'_>, local_name: &[u8]) -> bool {
    name.as_ref().rsplit(|byte| *byte == b':').next() == Some(local_name)
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<String, EpubError> {
    let mut file = archive
        .by_name(path)
        .map_err(|_| EpubError::MissingPackageDocument)?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|_| EpubError::InvalidPackageDocument)?;
    Ok(content)
}

fn read_zip_binary_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, EpubError> {
    let mut file = archive
        .by_name(path)
        .map_err(|_| EpubError::MissingPackageDocument)?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|_| EpubError::InvalidPackageDocument)?;
    Ok(content)
}

fn resolve_archive_path(package_path: &str, href: &str) -> String {
    let mut combined = PathBuf::new();

    if let Some(parent) = Path::new(package_path).parent() {
        combined.push(parent);
    }

    combined.push(href);

    let mut normalized = PathBuf::new();

    for component in combined.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {}
        }
    }

    normalized.to_string_lossy().replace('\\', "/")
}

fn cover_extension(media_type: &str, href: &str) -> Option<String> {
    match media_type {
        "image/jpeg" => Some("jpg".to_owned()),
        "image/png" => Some("png".to_owned()),
        "image/gif" => Some("gif".to_owned()),
        "image/webp" => Some("webp".to_owned()),
        _ => Path::new(href)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::parse_metadata;
    use crate::domain::publication::{Contributor, ContributorRole};

    #[test]
    fn parses_creator_and_role_refined_contributors() {
        let epub_bytes = build_epub(
            r##"<?xml version='1.0' encoding='UTF-8'?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata>
    <dc:identifier>urn:test:book</dc:identifier>
    <dc:title>Test Book</dc:title>
    <dc:creator id="creator-1">Primary Author</dc:creator>
    <dc:contributor id="contrib-1">Helpful Editor</dc:contributor>
    <meta property="role" refines="#contrib-1">edt</meta>
    <dc:contributor id="contrib-2">Skilled Translator</dc:contributor>
    <meta property="role" refines="#contrib-2">trl</meta>
  </metadata>
</package>"##,
        );

        let parsed = parse_metadata(&epub_bytes).expect("expected EPUB metadata to parse");

        assert_eq!(parsed.title, "Test Book");
        assert_eq!(parsed.source_identifier, "urn:test:book");
        assert_eq!(
            parsed.contributors,
            vec![
                Contributor {
                    name: "Primary Author".to_owned(),
                    role: ContributorRole::Author,
                },
                Contributor {
                    name: "Helpful Editor".to_owned(),
                    role: ContributorRole::Editor,
                },
                Contributor {
                    name: "Skilled Translator".to_owned(),
                    role: ContributorRole::Translator,
                },
            ]
        );
    }

    #[test]
    fn allows_epubs_without_contributors() {
        let epub_bytes = build_epub(
            r#"<?xml version='1.0' encoding='UTF-8'?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata>
    <dc:identifier>urn:test:no-contributors</dc:identifier>
    <dc:title>Contributor Free</dc:title>
  </metadata>
</package>"#,
        );

        let parsed = parse_metadata(&epub_bytes).expect("expected EPUB metadata to parse");

        assert!(parsed.contributors.is_empty());
    }

    #[test]
    fn falls_back_to_generic_contributor_for_unknown_roles() {
        let epub_bytes = build_epub(
            r##"<?xml version='1.0' encoding='UTF-8'?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata>
    <dc:identifier>urn:test:unknown-role</dc:identifier>
    <dc:title>Unknown Role</dc:title>
    <dc:contributor id="contrib-1">Mystery Person</dc:contributor>
    <meta property="role" refines="#contrib-1">zzz</meta>
  </metadata>
</package>"##,
        );

        let parsed = parse_metadata(&epub_bytes).expect("expected EPUB metadata to parse");

        assert_eq!(
            parsed.contributors,
            vec![Contributor {
                name: "Mystery Person".to_owned(),
                role: ContributorRole::Contributor,
            }]
        );
    }

    fn build_epub(package_document: &str) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();

        writer
            .start_file("META-INF/container.xml", options)
            .expect("expected container entry to be created");
        writer
            .write_all(
                br#"<?xml version='1.0' encoding='utf-8'?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
            )
            .expect("expected container document to be written");

        writer
            .start_file("OEBPS/content.opf", options)
            .expect("expected package entry to be created");
        writer
            .write_all(package_document.as_bytes())
            .expect("expected package document to be written");

        writer
            .finish()
            .expect("expected EPUB archive to finish")
            .into_inner()
    }
}
