use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

use crate::domain::publication::{
    AuthorSummary, Contributor, ContributorRole, NewPublication, OpdsPublicationSummary,
    PublicationDetail, PublicationSummary,
};

pub const DEFAULT_PAGE_SIZE: i64 = 20;
pub const MAX_PAGE_SIZE: i64 = 100;

#[derive(Clone, Copy, Debug)]
pub enum PublicationOrder {
    Title,
    Recent,
}

#[derive(Clone, Debug)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total_items: i64,
}

pub async fn list_authors(db: &PgPool) -> Result<Vec<AuthorSummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select
            lower(c.name) as author_key,
            min(c.name) as author_name,
            count(distinct pc.publication_id) as publication_count
        from contributors c
        join publication_contributors pc on pc.contributor_id = c.id
        where pc.role = 'author'
        group by lower(c.name)
        order by lower(min(c.name))
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AuthorSummary {
            key: row.get("author_key"),
            name: row.get("author_name"),
            publication_count: row.get("publication_count"),
        })
        .collect())
}

pub async fn list_publications_by_author(
    db: &PgPool,
    author_key: &str,
) -> Result<Vec<PublicationSummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select
            p.id,
            p.source_identifier,
            p.title,
            max(cover.storage_path) as cover_image_path,
            coalesce(
                string_agg(distinct c_all.name, ', ' order by c_all.name),
                'No contributors listed'
            ) as contributors
        from publications p
        join publication_contributors pc_author on pc_author.publication_id = p.id and pc_author.role = 'author'
        join contributors c_author on c_author.id = pc_author.contributor_id
        left join publication_contributors pc_all on pc_all.publication_id = p.id
        left join contributors c_all on c_all.id = pc_all.contributor_id
        left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
        where lower(c_author.name) = $1
        group by p.id
        order by lower(coalesce(p.sort_title, p.title)), p.created_at desc
        "#,
    )
    .bind(author_key)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(publication_summary_from_row).collect())
}

pub async fn find_author_name(
    db: &PgPool,
    author_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        select min(c.name)
        from contributors c
        join publication_contributors pc on pc.contributor_id = c.id
        where pc.role = 'author' and lower(c.name) = $1
        "#,
    )
    .bind(author_key)
    .fetch_optional(db)
    .await
}

pub async fn list_publications(db: &PgPool) -> Result<Vec<PublicationSummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select
            p.id,
            p.source_identifier,
            p.title,
            max(cover.storage_path) as cover_image_path,
            coalesce(
                string_agg(c.name, ', ' order by pc.position nulls last, c.name),
                'No contributors listed'
            ) as contributors
        from publications p
        left join publication_contributors pc on pc.publication_id = p.id
        left join contributors c on c.id = pc.contributor_id
        left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
        group by p.id
        order by lower(coalesce(p.sort_title, p.title)), p.created_at desc
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(publication_summary_from_row).collect())
}

pub async fn list_opds_publications(
    db: &PgPool,
    page: i64,
    per_page: i64,
    order: PublicationOrder,
) -> Result<PaginatedResult<OpdsPublicationSummary>, sqlx::Error> {
    let per_page = clamp_page_size(per_page);
    let page = clamp_page(page);
    let offset = (page - 1) * per_page;

    let total_items = sqlx::query_scalar::<_, i64>("select count(*) from publications")
        .fetch_one(db)
        .await?;

    let rows = match order {
        PublicationOrder::Title => {
            sqlx::query(
                r#"
                select
                    p.id,
                    p.source_identifier,
                    p.title,
                    p.updated_at,
                    max(cover.storage_path) as cover_image_path,
                    coalesce(
                        array_remove(array_agg(distinct case when pc.role = 'author' then c.name end), null),
                        '{}'::text[]
                    ) as authors
                from publications p
                left join publication_contributors pc on pc.publication_id = p.id
                left join contributors c on c.id = pc.contributor_id
                left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
                group by p.id
                order by lower(coalesce(p.sort_title, p.title)), p.created_at desc
                limit $1 offset $2
                "#,
            )
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await?
        }
        PublicationOrder::Recent => {
            sqlx::query(
                r#"
                select
                    p.id,
                    p.source_identifier,
                    p.title,
                    p.updated_at,
                    max(cover.storage_path) as cover_image_path,
                    coalesce(
                        array_remove(array_agg(distinct case when pc.role = 'author' then c.name end), null),
                        '{}'::text[]
                    ) as authors
                from publications p
                left join publication_contributors pc on pc.publication_id = p.id
                left join contributors c on c.id = pc.contributor_id
                left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
                group by p.id
                order by p.created_at desc, lower(coalesce(p.sort_title, p.title))
                limit $1 offset $2
                "#,
            )
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await?
        }
    };

    Ok(PaginatedResult {
        items: rows
            .into_iter()
            .map(opds_publication_summary_from_row)
            .collect(),
        total_items,
    })
}

pub async fn list_opds_publications_by_author(
    db: &PgPool,
    author_key: &str,
    page: i64,
    per_page: i64,
) -> Result<PaginatedResult<OpdsPublicationSummary>, sqlx::Error> {
    let per_page = clamp_page_size(per_page);
    let page = clamp_page(page);
    let offset = (page - 1) * per_page;

    let total_items = sqlx::query_scalar::<_, i64>(
        r#"
        select count(distinct p.id)
        from publications p
        join publication_contributors pc_author on pc_author.publication_id = p.id and pc_author.role = 'author'
        join contributors c_author on c_author.id = pc_author.contributor_id
        where lower(c_author.name) = $1
        "#,
    )
    .bind(author_key)
    .fetch_one(db)
    .await?;

    let rows = sqlx::query(
        r#"
        select
            p.id,
            p.source_identifier,
            p.title,
            p.updated_at,
            max(cover.storage_path) as cover_image_path,
            coalesce(
                array_remove(array_agg(distinct case when pc_all.role = 'author' then c_all.name end), null),
                '{}'::text[]
            ) as authors
        from publications p
        join publication_contributors pc_author on pc_author.publication_id = p.id and pc_author.role = 'author'
        join contributors c_author on c_author.id = pc_author.contributor_id
        left join publication_contributors pc_all on pc_all.publication_id = p.id
        left join contributors c_all on c_all.id = pc_all.contributor_id
        left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
        where lower(c_author.name) = $1
        group by p.id
        order by lower(coalesce(p.sort_title, p.title)), p.created_at desc
        limit $2 offset $3
        "#,
    )
    .bind(author_key)
    .bind(per_page)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(PaginatedResult {
        items: rows
            .into_iter()
            .map(opds_publication_summary_from_row)
            .collect(),
        total_items,
    })
}

pub async fn delete_publication(
    db: &PgPool,
    publication_id: uuid::Uuid,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let mut tx = db.begin().await?;

    let asset_paths = sqlx::query_scalar::<_, String>(
        r#"
        select storage_path
        from assets
        where publication_id = $1
        "#,
    )
    .bind(publication_id)
    .fetch_all(&mut *tx)
    .await?;

    let deleted_rows = sqlx::query(
        r#"
        delete from publications
        where id = $1
        "#,
    )
    .bind(publication_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if deleted_rows == 0 {
        tx.rollback().await?;
        return Ok(None);
    }

    sqlx::query(
        r#"
        delete from contributors c
        where not exists (
            select 1
            from publication_contributors pc
            where pc.contributor_id = c.id
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(asset_paths))
}

pub async fn find_publication_by_id(
    db: &PgPool,
    publication_id: uuid::Uuid,
) -> Result<Option<PublicationDetail>, sqlx::Error> {
    let Some(row) = sqlx::query(
        r#"
        select
            p.id,
            p.source_identifier,
            p.title,
            p.updated_at,
            max(cover.storage_path) as cover_image_path,
            max(cover.media_type) as cover_image_media_type,
            max(epub.storage_path) as epub_path,
            max(epub.partial_md5) as epub_partial_md5,
            max(epub.original_filename) as original_filename
        from publications p
        left join assets cover on cover.publication_id = p.id and cover.kind = 'cover_image'
        left join assets epub on epub.publication_id = p.id and epub.kind = 'primary_epub'
        where p.id = $1
        group by p.id
        "#,
    )
    .bind(publication_id)
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };

    let contributor_rows = sqlx::query(
        r#"
        select c.name, pc.role
        from publication_contributors pc
        join contributors c on c.id = pc.contributor_id
        where pc.publication_id = $1
        order by pc.position nulls last, c.name
        "#,
    )
    .bind(publication_id)
    .fetch_all(db)
    .await?;

    let title = row.get::<String, _>("title");

    Ok(Some(PublicationDetail {
        id: row.get("id"),
        source_identifier: row.get("source_identifier"),
        title_initial: title_initial(&title),
        title,
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
        contributors: contributor_rows
            .into_iter()
            .map(|contributor_row| Contributor {
                name: contributor_row.get("name"),
                role: ContributorRole::from_db_value(contributor_row.get("role")),
            })
            .collect(),
        cover_image_path: row.get("cover_image_path"),
        cover_image_media_type: row.get("cover_image_media_type"),
        epub_path: row.get("epub_path"),
        epub_partial_md5: row.get("epub_partial_md5"),
        original_filename: row.get("original_filename"),
    }))
}

pub async fn source_identifier_exists(
    db: &PgPool,
    source_identifier: &str,
) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        select exists(
            select 1
            from publications
            where source_identifier = $1
        )
        "#,
    )
    .bind(source_identifier)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

pub async fn create_publication(
    db: &PgPool,
    publication: &NewPublication,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query(
        r#"
        insert into publications (id, source_identifier, title, sort_title)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(publication.id)
    .bind(&publication.source_identifier)
    .bind(&publication.title)
    .bind(publication.title.to_lowercase())
    .execute(&mut *tx)
    .await?;

    for (position, contributor) in publication.contributors.iter().enumerate() {
        let contributor_id = uuid::Uuid::new_v4();

        sqlx::query(
            r#"
            insert into contributors (id, name)
            values ($1, $2)
            "#,
        )
        .bind(contributor_id)
        .bind(&contributor.name)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into publication_contributors (publication_id, contributor_id, role, position)
            values ($1, $2, $3, $4)
            "#,
        )
        .bind(publication.id)
        .bind(contributor_id)
        .bind(contributor.role.as_db_value())
        .bind(position as i32)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        insert into assets (
            id,
            publication_id,
            kind,
            storage_path,
            media_type,
            byte_size,
            partial_md5,
            original_filename
        )
        values ($1, $2, 'primary_epub', $3, $4, $5, $6, $7)
        "#,
    )
    .bind(publication.primary_asset.id)
    .bind(publication.id)
    .bind(&publication.primary_asset.storage_path)
    .bind(&publication.primary_asset.media_type)
    .bind(publication.primary_asset.byte_size)
    .bind(&publication.primary_asset.partial_md5)
    .bind(&publication.primary_asset.original_filename)
    .execute(&mut *tx)
    .await?;

    if let Some(cover_asset) = &publication.cover_asset {
        sqlx::query(
            r#"
            insert into assets (
                id,
                publication_id,
                kind,
                storage_path,
                media_type,
                byte_size,
                partial_md5,
                original_filename
            )
            values ($1, $2, 'cover_image', $3, $4, $5, $6, $7)
            "#,
        )
        .bind(cover_asset.id)
        .bind(publication.id)
        .bind(&cover_asset.storage_path)
        .bind(&cover_asset.media_type)
        .bind(cover_asset.byte_size)
        .bind(&cover_asset.partial_md5)
        .bind(&cover_asset.original_filename)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

fn publication_summary_from_row(row: sqlx::postgres::PgRow) -> PublicationSummary {
    let title = row.get::<String, _>("title");

    PublicationSummary {
        id: row.get("id"),
        source_identifier: row.get("source_identifier"),
        title_initial: title_initial(&title),
        cover_image_path: row.get("cover_image_path"),
        title,
        contributors: row.get("contributors"),
    }
}

fn opds_publication_summary_from_row(row: sqlx::postgres::PgRow) -> OpdsPublicationSummary {
    OpdsPublicationSummary {
        id: row.get("id"),
        source_identifier: row.get("source_identifier"),
        title: row.get("title"),
        authors: row.get::<Vec<String>, _>("authors"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
        cover_image_path: row.get("cover_image_path"),
    }
}

fn clamp_page(page: i64) -> i64 {
    if page < 1 { 1 } else { page }
}

fn clamp_page_size(per_page: i64) -> i64 {
    per_page.clamp(1, MAX_PAGE_SIZE)
}

fn title_initial(title: &str) -> String {
    title
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_owned())
}
