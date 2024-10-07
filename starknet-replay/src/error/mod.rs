//! This module contains all the errors returned by the `starknet-replay`
//! library.
//!
//! I am deriving only `Debug` and `Error` because not all inherited error types
//! implement `Clone` and `Eq`.
//!
//! Some libraries return [`anyhow::Error`]. Because it's not possible to
//! differentiate the origin of the error, [`anyhow::Error`] is transformed into
//! the `Unknown` type variant by implementing the `From<T>` trait.
//! In other cases, the error enum variant matches the library name from which
//! the error originates.

// Allowing `module_name_repetitions` is needed to make `clippy` happy and keep the suffix `Error`
// for all the error categories. Alternatively, shortening the name would limit expressiveness of
// the type in this case.
#![allow(clippy::module_name_repetitions)]

use thiserror::Error;

// If any error is added in the future, make sure to keep all sub-error enums as
// `pub` for ease of access.
pub use self::database::Error as DatabaseError;
pub use self::histogram::Error as HistogramError;
pub use self::permanent_state::Error as PermanentStateError;
pub use self::profiler::Error as ProfilerError;
pub use self::rpc_client::Error as RpcClientError;
pub use self::runner::Error as RunnerError;

mod database;
mod histogram;
mod permanent_state;
mod profiler;
mod rpc_client;
mod runner;

#[derive(Debug, Error)]
pub enum Error {
    /// `Error::Database` error is caused by issues quering the RPC endpoint.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// `Error::Histogram` error is caused by issues generating the libfunc
    /// histogram.
    #[error(transparent)]
    Histogram(#[from] HistogramError),

    /// `Error::Profiler` error is caused by issues with transaction profiling.
    #[error(transparent)]
    Profiler(#[from] ProfilerError),

    /// `Error::Runner` error is caused by issues with transaction replay.
    #[error(transparent)]
    Runner(#[from] RunnerError),
}
