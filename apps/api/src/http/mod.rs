//! HTTP layer: routing and cross-cutting middleware. Handlers stay thin and
//! delegate business logic to services.

mod health;

use std::time::Duration;

use axum::{
    Router,
    http::{HeaderName, Method, Request, StatusCode, header},
    routing::get,
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{error::ApiError, state::AppState};

/// Header carrying the request id; generated when the client sends none.
pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Options for [`build_app`] that come from configuration.
#[derive(Debug, Default, Clone)]
pub struct HttpOptions {
    pub cors_allowed_origins: Vec<axum::http::HeaderValue>,
}

/// Builds the full application router. Used by `main` and by the HTTP tests.
pub fn build_app(state: AppState, options: HttpOptions) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(options.cors_allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .expose_headers([REQUEST_ID_HEADER]);

    // `Router::layer` wraps everything added before it, so the last layer is
    // the outermost: CORS, then request id, then tracing, then the timeout.
    Router::new()
        .route("/health", get(health::health))
        .fallback(not_found)
        .with_state(state)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = request
                        .extensions()
                        .get::<RequestId>()
                        .and_then(|id| id.header_value().to_str().ok())
                        .unwrap_or_default();
                    // Path only: query strings could carry identifiers.
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        path = %request.uri().path(),
                        request_id,
                    )
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
        .layer(cors)
}

async fn not_found() -> ApiError {
    ApiError::NotFound
}
