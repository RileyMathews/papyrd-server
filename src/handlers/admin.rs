use std::collections::HashSet;

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::{Form, PrivateCookieJar};
use chrono::{Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth,
    domain::user::User,
    error::AppError,
    handlers::nav::NavView,
    permissions::Permission,
    repositories::{signup_invites, user_permissions, users},
    state::AppState,
};

#[derive(Template)]
#[template(path = "pages/admin_users.html")]
struct AdminUsersTemplate<'a> {
    users: &'a [User],
    nav: NavView,
    active_nav: &'static str,
}

#[derive(Template)]
#[template(path = "pages/admin_user_permissions.html")]
struct AdminUserPermissionsTemplate<'a> {
    user: &'a User,
    permission_options: Vec<PermissionOptionView>,
    saved: bool,
    nav: NavView,
    active_nav: &'static str,
}

#[derive(Template)]
#[template(path = "pages/admin_invites.html")]
struct AdminInvitesTemplate {
    invites: Vec<InviteView>,
    has_invites: bool,
    created: bool,
    expired: bool,
    nav: NavView,
    active_nav: &'static str,
}

struct PermissionOptionView {
    value: &'static str,
    label: &'static str,
    description: &'static str,
    checked: bool,
}

struct InviteView {
    id: Uuid,
    link: String,
    note: Option<String>,
    created_by: String,
    expires_at: String,
}

#[derive(Deserialize)]
pub struct PermissionQuery {
    saved: Option<String>,
}

#[derive(Deserialize)]
pub struct InvitesQuery {
    created: Option<String>,
    expired: Option<String>,
}

#[derive(Deserialize)]
pub struct PermissionForm {
    #[serde(default)]
    permissions: Vec<String>,
}

#[derive(Deserialize)]
pub struct InviteForm {
    note: Option<String>,
}

pub async fn users(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(current_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let current_permissions =
        user_permissions::permission_set_for_user(state.db(), current_user.id).await?;
    current_permissions.require(Permission::UserPermissionsEdit)?;
    let nav = NavView::from_permissions(&current_permissions);

    let users = users::list_users(state.db()).await?;
    let html = AdminUsersTemplate {
        users: &users,
        nav,
        active_nav: "admin",
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn edit_permissions_form(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<PermissionQuery>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some(current_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let current_permissions =
        user_permissions::permission_set_for_user(state.db(), current_user.id).await?;
    current_permissions.require(Permission::UserPermissionsEdit)?;
    let nav = NavView::from_permissions(&current_permissions);

    let Some(user) = users::find_user_by_id(state.db(), user_id).await? else {
        return Ok(Redirect::to("/admin/users").into_response());
    };
    let active_permissions =
        user_permissions::list_permissions_for_user(state.db(), user.id).await?;
    let permission_options = permission_options(&active_permissions);
    let html = AdminUserPermissionsTemplate {
        user: &user,
        permission_options,
        saved: query.saved.is_some(),
        nav,
        active_nav: "admin",
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn update_permissions(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    jar: PrivateCookieJar,
    Form(form): Form<PermissionForm>,
) -> Result<Response, AppError> {
    let Some(current_user) = auth::current_user(state.db(), &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };
    let current_permissions =
        user_permissions::permission_set_for_user(state.db(), current_user.id).await?;
    current_permissions.require(Permission::UserPermissionsEdit)?;

    let Some(user) = users::find_user_by_id(state.db(), user_id).await? else {
        return Ok(Redirect::to("/admin/users").into_response());
    };
    let permissions = parse_permissions(&form.permissions)?;
    user_permissions::set_permissions_for_user(state.db(), user.id, &permissions, current_user.id)
        .await?;

    let location = format!("/admin/users/{}/permissions?saved=1", user.id);
    Ok(Redirect::to(&location).into_response())
}

pub async fn invites(
    State(state): State<AppState>,
    Query(query): Query<InvitesQuery>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some((_current_user, nav)) = invite_admin_context(&state, &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let invites: Vec<InviteView> = signup_invites::list_open_invites(state.db())
        .await?
        .into_iter()
        .map(InviteView::from_invite)
        .collect();
    let has_invites = !invites.is_empty();
    let html = AdminInvitesTemplate {
        invites,
        has_invites,
        created: query.created.is_some(),
        expired: query.expired.is_some(),
        nav,
        active_nav: "invites",
    }
    .render()?;

    Ok(Html(html).into_response())
}

pub async fn create_invite(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<InviteForm>,
) -> Result<Response, AppError> {
    let Some((current_user, _nav)) = invite_admin_context(&state, &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    let note = normalize_note(form.note.as_deref());
    let expires_at = Utc::now() + Duration::seconds(state.invite_expiration_seconds());
    signup_invites::create_invite(
        state.db(),
        &new_invite_key(),
        note.as_deref(),
        current_user.id,
        expires_at,
    )
    .await?;

    Ok(Redirect::to("/admin/invites?created=1").into_response())
}

pub async fn expire_invite(
    State(state): State<AppState>,
    Path(invite_id): Path<Uuid>,
    jar: PrivateCookieJar,
) -> Result<Response, AppError> {
    let Some((_current_user, _nav)) = invite_admin_context(&state, &jar).await? else {
        return Ok(Redirect::to("/signin").into_response());
    };

    signup_invites::revoke_invite(state.db(), invite_id).await?;

    Ok(Redirect::to("/admin/invites?expired=1").into_response())
}

fn permission_options(active_permissions: &[Permission]) -> Vec<PermissionOptionView> {
    let active_permissions = active_permissions.iter().copied().collect::<HashSet<_>>();

    Permission::ALL
        .iter()
        .copied()
        .map(|permission| PermissionOptionView {
            value: permission.as_str(),
            label: permission.label(),
            description: permission.description(),
            checked: active_permissions.contains(&permission),
        })
        .collect()
}

fn parse_permissions(values: &[String]) -> Result<Vec<Permission>, AppError> {
    let mut permissions = Vec::new();

    for value in values {
        let Some(permission) = Permission::parse(value) else {
            return Err(AppError::BadRequest("Unknown permission selected."));
        };

        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }

    Ok(permissions)
}

impl InviteView {
    fn from_invite(invite: signup_invites::SignupInvite) -> Self {
        Self {
            id: invite.id,
            link: format!("/signup?invite={}", invite.invite_key),
            note: invite.note,
            created_by: invite
                .created_by_username
                .unwrap_or_else(|| "Deleted user".to_owned()),
            expires_at: invite.expires_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        }
    }
}

async fn invite_admin_context(
    state: &AppState,
    jar: &PrivateCookieJar,
) -> Result<Option<(User, NavView)>, AppError> {
    let Some(current_user) = auth::current_user(state.db(), jar).await? else {
        return Ok(None);
    };
    let current_permissions =
        user_permissions::permission_set_for_user(state.db(), current_user.id).await?;
    current_permissions.require(Permission::UserInvitesCreate)?;
    let nav = NavView::from_permissions(&current_permissions);

    Ok(Some((current_user, nav)))
}

fn normalize_note(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn new_invite_key() -> String {
    Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::FromRequest,
        http::{Request, header::CONTENT_TYPE},
    };

    use super::*;

    #[tokio::test]
    async fn permission_form_deserializes_single_checkbox_value() {
        let request = Request::builder()
            .method("POST")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("permissions=user.permissions.edit"))
            .unwrap();

        let Form(form) = Form::<PermissionForm>::from_request(request, &())
            .await
            .unwrap();

        assert_eq!(form.permissions, vec!["user.permissions.edit"]);
    }
}
