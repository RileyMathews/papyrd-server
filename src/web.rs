use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{Method, header},
    routing::{get, post},
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};

use crate::{handlers, state::AppState};

const UPLOAD_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::books::index))
        .route("/books", get(handlers::books::index))
        .route("/books/{id}", get(handlers::books::show))
        .route("/authors", get(handlers::authors::index))
        .route("/authors/{author_key}", get(handlers::authors::show))
        .route("/opds", get(handlers::opds::root))
        .route("/opds/publications", get(handlers::opds::publications_feed))
        .route(
            "/opds/publications/recent",
            get(handlers::opds::recent_publications_feed),
        )
        .route("/opds/authors", get(handlers::opds::authors_feed))
        .route(
            "/opds/authors/{author_key}",
            get(handlers::opds::author_publications_feed),
        )
        .route(
            "/opds/publications/{id}",
            get(handlers::opds::publication_document),
        )
        .route("/books/{id}/cover", get(handlers::assets::cover_image))
        .route("/books/{id}/download", get(handlers::assets::download_epub))
        .route("/users/create", post(handlers::kosync::register))
        .route("/users/auth", get(handlers::kosync::authorize))
        .route(
            "/syncs/progress",
            axum::routing::put(handlers::kosync::update_progress),
        )
        .route(
            "/syncs/progress/{document}",
            get(handlers::kosync::get_progress),
        )
        .route("/healthcheck", get(handlers::kosync::healthcheck))
        .route("/books/{id}/delete", post(handlers::books::delete))
        .route(
            "/signup",
            get(handlers::auth::signup_form).post(handlers::auth::signup),
        )
        .route(
            "/signin",
            get(handlers::auth::signin_form).post(handlers::auth::signin),
        )
        .route("/signout", post(handlers::auth::signout))
        .route(
            "/upload",
            get(handlers::upload::upload_form)
                .post(handlers::upload::upload)
                .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT_BYTES)),
        )
        .nest_service("/static", ServeDir::new("static"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers([header::ACCEPT, header::AUTHORIZATION, header::CONTENT_TYPE])
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::OPTIONS,
                    Method::HEAD,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                ]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
