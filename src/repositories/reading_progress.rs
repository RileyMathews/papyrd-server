use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::reading_progress::ReadingProgress;

pub async fn find_by_user_and_document(
    db: &PgPool,
    user_id: Uuid,
    document: &str,
) -> Result<Option<ReadingProgress>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        select user_id, document, progress, percentage, device, device_id, updated_at
        from reading_progress
        where user_id = $1 and document = $2
        "#,
    )
    .bind(user_id)
    .bind(document)
    .fetch_optional(db)
    .await?;

    Ok(row.map(reading_progress_from_row))
}

pub async fn upsert(
    db: &PgPool,
    progress: &ReadingProgress,
) -> Result<ReadingProgress, sqlx::Error> {
    let row = sqlx::query(
        r#"
        insert into reading_progress (user_id, document, progress, percentage, device, device_id)
        values ($1, $2, $3, $4, $5, $6)
        on conflict (user_id, document)
        do update set
            progress = excluded.progress,
            percentage = excluded.percentage,
            device = excluded.device,
            device_id = excluded.device_id,
            updated_at = now()
        returning user_id, document, progress, percentage, device, device_id, updated_at
        "#,
    )
    .bind(progress.user_id)
    .bind(&progress.document)
    .bind(&progress.progress)
    .bind(progress.percentage)
    .bind(&progress.device)
    .bind(&progress.device_id)
    .fetch_one(db)
    .await?;

    Ok(reading_progress_from_row(row))
}

fn reading_progress_from_row(row: sqlx::postgres::PgRow) -> ReadingProgress {
    ReadingProgress {
        user_id: row.get("user_id"),
        document: row.get("document"),
        progress: row.get("progress"),
        percentage: row.get("percentage"),
        device: row.get("device"),
        device_id: row.get("device_id"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    }
}
