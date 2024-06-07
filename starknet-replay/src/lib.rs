//! The library `starknet-replay` replays transactions from the `pathfinder`
//! sqlite database and collects statistics on the execution of those
//! transactions.
//!
//! At the current time, the library focuses on gathering usage
//! statistics of the various library functions (libfuncs) in the
//! blocks being replayed. In the future it may be expanded to
//! collect more kinds of data during replay.
//!
//! The simplest interaction with this library is to call the function
//! [`run_replay`] which returns the usage statistics of libfuncs.
//!
//! The key structs of the library are as follows:
//!
//! - [`ReplayBlock`] struct which contains a single block of transactions.
//! - [`profiler::SierraProfiler`] struct to extract profiling data from a list
//!   of visited program counters.
//! - [`profiler::replace_ids::DebugReplacer`] struct replaces the ids of
//!   libfuncs and types in a Sierra program.
//!
//! Beyond [`run_replay`], these are the other key public functions of the
//! library:
//!
//! - [`profiler::analysis::extract_libfuncs_weight`] which updates the
//!   cumulative usage of libfuncs
//! - [`profiler::replace_ids::replace_sierra_ids_in_program`] which replaces
//!   the ids of libfuncs and types with their debug name in a Sierra program.
//! - [`histogram::export`] which generates the histogram of libfunc frequencies
//!   and exports it as SVG image.

#![warn(
    clippy::all,
    clippy::cargo,
    clippy::pedantic,
    clippy::missing_docs_in_private_items
)]
#![allow(clippy::multiple_crate_versions)] // Due to duplicate dependencies in pathfinder

use error::RunnerError;
use runner::replay_block::ReplayBlock;

pub use crate::histogram::export as export_histogram;
pub use crate::profiler::replay_statistics::ReplayStatistics;
pub use crate::profiler::report::write_to_file;
pub use crate::runner::pathfinder_db::{connect_to_database, get_latest_block_number};
pub use crate::runner::replay_range::ReplayRange;
pub use crate::runner::run_replay;

pub mod error;
pub mod histogram;
pub mod profiler;
pub mod runner;
