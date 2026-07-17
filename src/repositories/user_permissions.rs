use sqlx::{Executor, PgPool, Postgres, Row};
use uuid::Uuid;

use crate::permissions::{Permission, PermissionSet};

pub async fn list_permissions_for_user(
    db: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Permission>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        select permission::text as permission
        from user_permissions
        where user_id = $1
        order by permission
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| Permission::parse(row.get("permission")))
        .collect())
}

pub async fn permission_set_for_user(
    db: &PgPool,
    user_id: Uuid,
) -> Result<PermissionSet, sqlx::Error> {
    let permissions = list_permissions_for_user(db, user_id).await?;

    Ok(PermissionSet::new(permissions))
}

pub async fn grant_permissions<'e, E>(
    executor: E,
    user_id: Uuid,
    permissions: &[Permission],
    granted_by_user_id: Option<Uuid>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    if permissions.is_empty() {
        return Ok(());
    }

    let permission_values = permissions
        .iter()
        .map(|permission| permission.as_str())
        .collect::<Vec<_>>();

    sqlx::query(
        r#"
        insert into user_permissions (user_id, permission, granted_by_user_id)
        select $1, permissions.permission::permission, $3
        from unnest($2::text[]) as permissions(permission)
        on conflict do nothing
        "#,
    )
    .bind(user_id)
    .bind(permission_values)
    .bind(granted_by_user_id)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn set_permissions_for_user(
    db: &PgPool,
    user_id: Uuid,
    permissions: &[Permission],
    granted_by_user_id: Uuid,
) -> Result<(), sqlx::Error> {
    let mut transaction = db.begin().await?;

    sqlx::query("delete from user_permissions where user_id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;

    grant_permissions(
        &mut *transaction,
        user_id,
        permissions,
        Some(granted_by_user_id),
    )
    .await?;

    transaction.commit().await
}
