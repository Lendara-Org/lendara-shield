//! Shared state handed to every handler.

use sqlx::PgPool;

/// Cheap to clone: every field is reference-counted.
#[derive(Debug, Clone)]
pub struct AppState {
    pub db: PgPool,
}
