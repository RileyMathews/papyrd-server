use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres, Row};
use uuid::Uuid;

use crate::domain::user::User;

pub struct StoredUser {
    pub user: User,
    pub password_hash: String,
    pub kosync_userkey_hash: Option<String>,
}

pub async fn create_user<'e, E>(
    executor: E,
    username: &str,
    normalized_username: &str,
    password_hash: &str,
    kosync_userkey_hash: &str,
) -> Result<StoredUser, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
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
    .fetch_one(executor)
    .await?;

    Ok(stored_user_from_row(row))
}

pub async fn list_users(db: &PgPool) -> Result<Vec<User>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select id, username, normalized_username, created_at, updated_at
        from users
        order by lower(username), created_at
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(|row| user_from_row(&row)).collect())
}

pub async fn has_any_users(db: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("select exists(select 1 from users)")
        .fetch_one(db)
        .await
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

    Ok(row.map(|row| user_from_row(&row)))
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
        user: user_from_row(&row),
        password_hash: row.get("password_hash"),
        kosync_userkey_hash: row.get("kosync_userkey_hash"),
    }
}

fn user_from_row(row: &sqlx::postgres::PgRow) -> User {
    User {
        id: row.get("id"),
        username: row.get("username"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    }
}
