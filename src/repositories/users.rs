use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::user::User;

pub struct StoredUser {
    pub user: User,
    pub password_hash: String,
    pub kosync_userkey_hash: Option<String>,
}

pub async fn create_user(
    db: &PgPool,
    username: &str,
    normalized_username: &str,
    password_hash: &str,
    kosync_userkey_hash: &str,
) -> Result<StoredUser, sqlx::Error> {
    let row = sqlx::query(
        r#"
        insert into users (username, normalized_username, password_hash, kosync_userkey_hash)
        values ($1, $2, $3, $4)
        returning id, username, normalized_username, password_hash, kosync_userkey_hash, created_at, updated_at
        "#,
    )
    .bind(username)
    .bind(normalized_username)
    .bind(password_hash)
    .bind(kosync_userkey_hash)
    .fetch_one(db)
    .await?;

    Ok(stored_user_from_row(row))
}

pub async fn find_user_by_id(db: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        select id, username, normalized_username, password_hash, kosync_userkey_hash, created_at, updated_at
        from users
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(stored_user_from_row).map(|stored| stored.user))
}

pub async fn find_user_by_normalized_username(
    db: &PgPool,
    normalized_username: &str,
) -> Result<Option<StoredUser>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        select id, username, normalized_username, password_hash, kosync_userkey_hash, created_at, updated_at
        from users
        where normalized_username = $1
        "#,
    )
    .bind(normalized_username)
    .fetch_optional(db)
    .await?;

    Ok(row.map(stored_user_from_row))
}

fn stored_user_from_row(row: sqlx::postgres::PgRow) -> StoredUser {
    StoredUser {
        user: User {
            id: row.get("id"),
            username: row.get("username"),
            created_at: row.get::<DateTime<Utc>, _>("created_at"),
            updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
        },
        password_hash: row.get("password_hash"),
        kosync_userkey_hash: row.get("kosync_userkey_hash"),
    }
}
