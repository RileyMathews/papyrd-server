use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres, Row};
use uuid::Uuid;

pub struct SignupInvite {
    pub id: Uuid,
    pub invite_key: String,
    pub note: Option<String>,
    pub created_by_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn create_invite(
    db: &PgPool,
    invite_key: &str,
    note: Option<&str>,
    created_by_user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        insert into signup_invites (invite_key, note, created_by_user_id, expires_at)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(invite_key)
    .bind(note)
    .bind(created_by_user_id)
    .bind(expires_at)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn list_open_invites(db: &PgPool) -> Result<Vec<SignupInvite>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select
            signup_invites.id,
            signup_invites.invite_key,
            signup_invites.note,
            signup_invites.created_at,
            signup_invites.expires_at,
            users.username as created_by_username
        from signup_invites
        left join users on users.id = signup_invites.created_by_user_id
        where signup_invites.revoked_at is null
            and signup_invites.used_at is null
            and signup_invites.expires_at > now()
        order by signup_invites.created_at desc
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(invite_from_row).collect())
}

pub async fn is_open_invite_key(db: &PgPool, invite_key: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r#"
        select exists(
            select 1
            from signup_invites
            where invite_key = $1
                and revoked_at is null
                and used_at is null
                and expires_at > now()
        )
        "#,
    )
    .bind(invite_key)
    .fetch_one(db)
    .await
}

pub async fn claim_invite<'e, E>(
    executor: E,
    invite_key: &str,
    used_by_user_id: Uuid,
) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query(
        r#"
        update signup_invites
        set used_at = now(), used_by_user_id = $2
        where invite_key = $1
            and revoked_at is null
            and used_at is null
            and expires_at > now()
        returning id
        "#,
    )
    .bind(invite_key)
    .bind(used_by_user_id)
    .fetch_optional(executor)
    .await?;

    Ok(row.is_some())
}

pub async fn revoke_invite(db: &PgPool, invite_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        update signup_invites
        set revoked_at = now()
        where id = $1
            and revoked_at is null
            and used_at is null
            and expires_at > now()
        "#,
    )
    .bind(invite_id)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

fn invite_from_row(row: sqlx::postgres::PgRow) -> SignupInvite {
    SignupInvite {
        id: row.get("id"),
        invite_key: row.get("invite_key"),
        note: row.get("note"),
        created_by_username: row.get("created_by_username"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
    }
}
