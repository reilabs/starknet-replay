//! This module contains all the errors returned by the `cairo-replay` library.
//!
//! I am deriving only `Debug` and `Error` because not all inherited error types
//! implement `Clone` and `Eq`.
//!
//! Some libraries return `anyhow::Error`. Because it's not possible to
//! differentiate the origin of the error, `anyhow::Error` is transformed into
//! the `Unknown` type variant by implementing the `From<T>` trait.
//! In other cases, the error enum variant matches the library name from which
//! the error originates.

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
    /// `Error::Database` error is caused by issues quering the
    /// Pathfinder database.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// `Error::Runner` error is caused by issues with transaction
    /// replay or profiling.
    #[error(transparent)]
    Runner(#[from] RunnerError),
}
