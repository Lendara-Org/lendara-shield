//! `GET /health`: liveness of the service and its database.

use std::time::Duration;

use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::state::AppState;

const DB_PING_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Degraded,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Ok,
    Unreachable,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: Status,
    pub database: DependencyStatus,
}

/// 200 when every dependency answers, 503 otherwise, so load balancers and
/// `docker compose` healthchecks can rely on the status code alone.
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let ping = tokio::time::timeout(
        DB_PING_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db),
    )
    .await;

    match ping {
        Ok(Ok(_)) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: Status::Ok,
                database: DependencyStatus::Ok,
            }),
        ),
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "health check: database query failed");
            degraded()
        }
        Err(_) => {
            tracing::warn!("health check: database ping timed out");
            degraded()
        }
    }
}

fn degraded() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(HealthResponse {
            status: Status::Degraded,
            database: DependencyStatus::Unreachable,
        }),
    )
}
