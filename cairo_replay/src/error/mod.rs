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

#[derive(Clone, Debug, Error)]
pub enum CairoReplayError {
    #[error(transparent)]
    Database(#[from] DatabaseError),

    #[error(transparent)]
    Runner(#[from] RunnerError),
}
