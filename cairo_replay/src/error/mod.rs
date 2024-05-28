//! This module contains all the errors returned by the `cairo-replay` library.
//! I am deriving only `Debug` and `Error` because not all inherited error types
//! implement `Clone` and `Eq`.

// Allowing `module_name_repetitions` helps to keep `DatabaseError` and
// `RunnerError`. Alternatively, shortening the name would limit expressiveness
// of the type in this case.
#![allow(clippy::module_name_repetitions)]

use thiserror::Error;

// Keep all sub-error enums as `pub` for ease of access
pub use self::database::Error as DatabaseError;
pub use self::runner::Error as RunnerError;

pub mod database;
pub mod runner;

#[derive(Debug, Error)]
pub enum Error {
    /// `CairoReplayError::Database` error is caused by issues quering the
    /// Pathfinder database.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// `CairoReplayError::Runner` error is caused by issues with transaction
    /// replay or profiling.
    #[error(transparent)]
    Runner(#[from] RunnerError),
}
