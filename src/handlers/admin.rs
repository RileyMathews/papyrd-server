use std::collections::HashSet;

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth,
    domain::user::User,
    error::AppError,
    handlers::nav::NavView,
    permissions::Permission,
    repositories::{user_permissions, users},
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

struct PermissionOptionView {
    input_id: String,
    value: &'static str,
    label: &'static str,
    description: &'static str,
    checked: bool,
}

#[derive(Deserialize)]
pub struct PermissionQuery {
    saved: Option<String>,
}

#[derive(Deserialize)]
pub struct PermissionForm {
    #[serde(default)]
    permissions: Vec<String>,
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

fn permission_options(active_permissions: &[Permission]) -> Vec<PermissionOptionView> {
    let active_permissions = active_permissions.iter().copied().collect::<HashSet<_>>();

    Permission::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, permission)| PermissionOptionView {
            input_id: format!("permission-{index}"),
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
        let Some(permission) = Permission::from_str(value) else {
            return Err(AppError::BadRequest("Unknown permission selected."));
        };

        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }

    Ok(permissions)
}
