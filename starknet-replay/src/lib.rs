//! The library `starknet-replay` replays transactions from the Starknet
//! blockchain and collects statistics on the execution of those transactions.
//!
//! At the current time, the library focuses on gathering usage
//! statistics of the various library functions (libfuncs) in the
//! blocks being replayed. In the future it may be expanded to
//! collect more kinds of data during replay.
//!
//! The simplest interaction with this library is to call the function
//! [`crate::runner::run_replay`] which returns the usage statistics of
//! libfuncs.
//!
//! The key structs of the library are as follows:
//!
//! - [`ReplayBlock`] struct which contains a single block of transactions.
//! - [`profiler::SierraProfiler`] struct to extract profiling data from a list
//!   of visited program counters.
//! - [`profiler::replace_ids::DebugReplacer`] struct replaces the ids of
//!   libfuncs and types in a Sierra program.
//!
//! Beyond [`crate::runner::run_replay`], these are the other key public
//! functions of the library:
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
    clippy::missing_docs_in_private_items,
    clippy::unwrap_used
)]
#![allow(clippy::multiple_crate_versions)] // Different libraries depend on different versions of the same library.

use error::RunnerError;
use runner::replay_block::ReplayBlock;

pub mod block_number;
pub mod error;
pub mod histogram;
pub mod profiler;
pub mod runner;
pub mod storage;
