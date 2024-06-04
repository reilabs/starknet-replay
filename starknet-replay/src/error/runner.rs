//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use pathfinder_executor::TransactionExecutionError;
use thiserror::Error;

use crate::error::DatabaseError;

#[derive(Debug, Error)]
pub enum Error {
    /// `PathfinderExecutor` is for errors reported by the crate
    /// `pathfinder_executor`.
    #[error(transparent)]
    PathfinderExecutor(#[from] TransactionExecutionError),

    /// `GenerateReplayWork` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::generate_replay_work`.
    #[error(transparent)]
    GenerateReplayWork(anyhow::Error),

    /// `ReplayBlocks` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::replay_blocks`.
    #[error(transparent)]
    ReplayBlocks(anyhow::Error),

    /// `ExecuteBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::execute_block`.
    #[error(transparent)]
    ExecuteBlock(anyhow::Error),

    /// `DatabaseAccess` is used to convert from `DatabaseError` into
    /// `RunnerError` when database functions are called in the module `runner`.
    #[error(transparent)]
    DatabaseAccess(#[from] DatabaseError),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error during block replay: {0:?}")]
    Unknown(String),
}
