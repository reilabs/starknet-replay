use thiserror::Error;

// Keep all sub-error enums as `pub` for ease of access
pub use self::database_error::DatabaseError;
pub use self::runner_error::RunnerError;

pub mod database_error;
pub mod runner_error;

#[derive(Clone, Debug, Error)]
pub enum CairoReplayError {
    #[error(transparent)]
    Database(#[from] DatabaseError),

    #[error(transparent)]
    Runner(#[from] RunnerError),
}
