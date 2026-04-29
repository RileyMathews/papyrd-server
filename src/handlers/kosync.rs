use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth,
    domain::{reading_progress::ReadingProgress, user::User},
    error::AppError,
    repositories::reading_progress,
    state::AppState,
};

const KOSYNC_MEDIA_TYPE: &str = "application/vnd.koreader.v1+json";
const KOSYNC_AUTH_USER_HEADER: &str = "x-auth-user";
const KOSYNC_AUTH_KEY_HEADER: &str = "x-auth-key";

const ERROR_UNAUTHORIZED_USER: u16 = 2001;
const ERROR_INVALID_FIELDS: u16 = 2003;
const ERROR_DOCUMENT_FIELD_MISSING: u16 = 2004;
const ERROR_USER_REGISTRATION_DISABLED: u16 = 2005;

#[derive(Deserialize)]
pub struct UpdateProgressRequest {
    document: String,
    progress: String,
    percentage: f64,
    device: String,
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Serialize)]
struct AuthorizeResponse<'a> {
    authorized: &'a str,
}

#[derive(Serialize)]
struct HealthcheckResponse<'a> {
    state: &'a str,
}

#[derive(Serialize)]
struct UpdateProgressResponse<'a> {
    document: &'a str,
    timestamp: i64,
}

#[derive(Serialize)]
struct GetProgressResponse {
    document: String,
    percentage: f64,
    progress: String,
    device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    timestamp: i64,
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    code: u16,
    message: &'a str,
}

#[derive(Serialize)]
struct EmptyResponse {}

pub async fn register() -> Result<Response<axum::body::Body>, AppError> {
    kosync_json_response(
        StatusCode::PAYMENT_REQUIRED,
        &ErrorResponse {
            code: ERROR_USER_REGISTRATION_DISABLED,
            message: "User registration is disabled.",
        },
    )
}

pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response<axum::body::Body>, AppError> {
    if require_kosync_user(state, &headers).await?.is_none() {
        return kosync_unauthorized_response();
    }

    kosync_json_response(StatusCode::OK, &AuthorizeResponse { authorized: "OK" })
}

pub async fn update_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProgressRequest>,
) -> Result<Response<axum::body::Body>, AppError> {
    let Some(user) = require_kosync_user(state.clone(), &headers).await? else {
        return kosync_unauthorized_response();
    };

    if !valid_document(&request.document) {
        return kosync_json_response(
            StatusCode::FORBIDDEN,
            &ErrorResponse {
                code: ERROR_DOCUMENT_FIELD_MISSING,
                message: "Field 'document' not provided.",
            },
        );
    }

    if !valid_progress_update(&request) {
        return kosync_json_response(
            StatusCode::FORBIDDEN,
            &ErrorResponse {
                code: ERROR_INVALID_FIELDS,
                message: "Invalid request",
            },
        );
    }

    let stored = reading_progress::upsert(
        state.db(),
        &ReadingProgress {
            user_id: user.id,
            document: request.document,
            progress: request.progress,
            percentage: request.percentage,
            device: request.device,
            device_id: request.device_id.filter(|value| !value.trim().is_empty()),
            updated_at: chrono::Utc::now(),
        },
    )
    .await?;

    kosync_json_response(
        StatusCode::OK,
        &UpdateProgressResponse {
            document: &stored.document,
            timestamp: stored.updated_at.timestamp(),
        },
    )
}

pub async fn get_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(document): Path<String>,
) -> Result<Response<axum::body::Body>, AppError> {
    let Some(user) = require_kosync_user(state.clone(), &headers).await? else {
        return kosync_unauthorized_response();
    };

    if !valid_document(&document) {
        return kosync_json_response(
            StatusCode::FORBIDDEN,
            &ErrorResponse {
                code: ERROR_DOCUMENT_FIELD_MISSING,
                message: "Field 'document' not provided.",
            },
        );
    }

    let Some(progress) =
        reading_progress::find_by_user_and_document(state.db(), user.id, &document).await?
    else {
        return kosync_json_response(StatusCode::OK, &EmptyResponse {});
    };

    kosync_json_response(
        StatusCode::OK,
        &GetProgressResponse {
            document: progress.document,
            percentage: progress.percentage,
            progress: progress.progress,
            device: progress.device,
            device_id: progress.device_id,
            timestamp: progress.updated_at.timestamp(),
        },
    )
}

pub async fn healthcheck() -> Result<Response<axum::body::Body>, AppError> {
    kosync_json_response(StatusCode::OK, &HealthcheckResponse { state: "OK" })
}

async fn require_kosync_user(
    state: AppState,
    headers: &HeaderMap,
) -> Result<Option<User>, AppError> {
    auth::kosync_auth_user(
        state.db(),
        headers.get(KOSYNC_AUTH_USER_HEADER),
        headers.get(KOSYNC_AUTH_KEY_HEADER),
    )
    .await
}

fn valid_document(document: &str) -> bool {
    !document.trim().is_empty()
}

fn valid_progress_update(request: &UpdateProgressRequest) -> bool {
    !request.progress.is_empty()
        && !request.device.trim().is_empty()
        && request.percentage.is_finite()
        && (0.0..=1.0).contains(&request.percentage)
}

fn kosync_unauthorized_response() -> Result<Response<axum::body::Body>, AppError> {
    kosync_json_response(
        StatusCode::UNAUTHORIZED,
        &ErrorResponse {
            code: ERROR_UNAUTHORIZED_USER,
            message: "Unauthorized",
        },
    )
}

fn kosync_json_response<T: Serialize>(
    status: StatusCode,
    value: &T,
) -> Result<Response<axum::body::Body>, AppError> {
    let body =
        serde_json::to_vec(value).map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut response = Response::new(axum::body::Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(KOSYNC_MEDIA_TYPE),
    );
    Ok(response)
}
