//! HTTP layer: routing and cross-cutting middleware. Handlers stay thin and
//! delegate business logic to services.

mod audit;
mod extract;
mod health;
mod info;
mod intents;
pub mod rate_limit;
mod users;

use std::time::Duration;

use axum::{
    Router,
    extract::MatchedPath,
    http::{HeaderName, Method, Request, StatusCode, header},
    middleware,
    routing::{get, post},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub use info::InfoResponse;
use rate_limit::{RateLimit, RateLimiter};

use crate::{
    adapters::soroban::rpc::REQUEST_TIMEOUT as RPC_TIMEOUT, asp::service::CONFIRM_TIMEOUT,
    error::ApiError, state::AppState,
};

/// Header carrying the request id; generated when the client sends none.
pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Timeout of the routes that write to the ASP (onboarding, block, unblock).
/// They wait for the chain for up to [`CONFIRM_TIMEOUT`] in total and may
/// then finish one step already started (sequence, simulation, submit and
/// status: four RPC calls), plus [`ASP_REQUEST_SLACK`] for the database.
/// The HTTP layer must not cut them earlier: a cut request drops its future
/// between storing a transaction and sending it.
pub const ASP_REQUEST_TIMEOUT: Duration = Duration::from_secs(
    CONFIRM_TIMEOUT.as_secs() + 4 * RPC_TIMEOUT.as_secs() + ASP_REQUEST_SLACK.as_secs(),
);

/// Margin of [`ASP_REQUEST_TIMEOUT`] for database work around the RPC calls.
const ASP_REQUEST_SLACK: Duration = Duration::from_secs(15);

/// Options for [`build_app`] that come from configuration.
#[derive(Debug, Default, Clone)]
pub struct HttpOptions {
    pub cors_allowed_origins: Vec<axum::http::HeaderValue>,
    /// Per-client limit on onboarding and intent creation; `None` disables it.
    pub rate_limit: Option<RateLimit>,
}

/// Builds the full application router. Used by `main` and by the HTTP tests.
pub fn build_app(state: AppState, options: HttpOptions) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(options.cors_allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .expose_headers([REQUEST_ID_HEADER]);

    let limiter = RateLimiter::new(options.rate_limit);
    let rate_limited = || middleware::from_fn_with_state(limiter.clone(), rate_limit::limit);

    // ASP writes wait for the chain; everything else answers quickly.
    let asp_writes = Router::new()
        .route("/v1/onboarding", post(users::onboard).layer(rate_limited()))
        .route("/v1/asp/block", post(users::block))
        .route("/v1/asp/unblock", post(users::unblock))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            ASP_REQUEST_TIMEOUT,
        ));

    // `Router::layer` wraps everything added before it, so the last layer is
    // the outermost: CORS, then request id, then tracing, then the timeout.
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/info", get(info::info))
        .route("/v1/users/{account}", get(users::get_user))
        .route("/v1/intents", post(intents::create).layer(rate_limited()))
        .route("/v1/intents/{id}", get(intents::get))
        .route("/v1/audit/transactions", get(audit::transactions))
        .route("/v1/audit/public/{tx_hash}", get(audit::public))
        .fallback(not_found)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .merge(asp_writes)
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = request
                        .extensions()
                        .get::<RequestId>()
                        .and_then(|id| id.header_value().to_str().ok())
                        .unwrap_or_default();
                    // Route template (`/v1/users/{account}`), never the raw
                    // path: it would put user addresses in the logs (§0.5).
                    let route = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map_or("unmatched", MatchedPath::as_str);
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        route,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asp_routes_outlast_the_wait_for_the_chain() {
        // The budget plus the RPC calls one ASP step can still be making,
        // with room to spare: equal would cut a request that is on time.
        assert!(ASP_REQUEST_TIMEOUT > CONFIRM_TIMEOUT + RPC_TIMEOUT * 4);
        assert!(ASP_REQUEST_TIMEOUT > REQUEST_TIMEOUT);
    }
}
