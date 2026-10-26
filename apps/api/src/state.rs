//! Shared state handed to every handler.

use std::sync::Arc;

use secrecy::SecretString;
use sqlx::PgPool;

use crate::{asp::service::AspService, http::InfoResponse};

/// Cheap to clone: every field is reference-counted.
#[derive(Debug, Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Public configuration, computed once at startup.
    pub info: Arc<InfoResponse>,
    pub asp: AspService,
    /// Bearer token for the admin endpoints; `None` disables them.
    pub admin_token: Arc<Option<SecretString>>,
}
