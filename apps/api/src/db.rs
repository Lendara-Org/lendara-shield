//! Postgres connection pool and embedded migrations.

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

/// Migrations under `apps/api/migrations`, embedded at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const MAX_CONNECTIONS: u32 = 10;
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the pool and checks that the database is reachable.
pub async fn connect(database_url: &SecretString) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(database_url.expose_secret())
        .await
}
