//! Connection pool and migrations against a real Postgres (`DATABASE_URL`).

use secrecy::SecretString;
use shield_api::db;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database_url() -> Result<SecretString, std::env::VarError> {
    std::env::var("DATABASE_URL").map(SecretString::from)
}

#[tokio::test]
async fn connects_and_applies_migrations_idempotently() -> TestResult {
    let pool = db::connect(&database_url()?).await?;

    // Running twice must be a no-op the second time (startup and `migrate`).
    db::MIGRATOR.run(&pool).await?;
    db::MIGRATOR.run(&pool).await?;

    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await?;
    assert_eq!(applied, i64::try_from(db::MIGRATOR.iter().count())?);
    Ok(())
}

#[tokio::test]
async fn fails_within_the_acquire_timeout_when_unreachable() {
    let url = SecretString::from("postgres://shield:shield@127.0.0.1:1/shield");
    assert!(db::connect(&url).await.is_err());
}
