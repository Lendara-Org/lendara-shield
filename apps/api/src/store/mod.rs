//! Postgres repositories: one function per query. Rows are converted to
//! validated domain types; a row that does not parse is a bug and surfaces
//! as [`StoreError::Corrupt`].

pub mod asp_events;
pub mod intents;
pub mod users;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("corrupt row in {table}: {reason}")]
    Corrupt { table: &'static str, reason: String },
}

/// Name of the constraint a unique violation hit, if that is the error.
pub(crate) fn unique_violation(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db) if db.is_unique_violation() => db.constraint(),
        _ => None,
    }
}

pub(crate) fn corrupt(table: &'static str, reason: impl std::fmt::Display) -> StoreError {
    StoreError::Corrupt {
        table,
        reason: reason.to_string(),
    }
}
