//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use std::num::TryFromIntError;

use blockifier::state::errors::StateError;
use blockifier::transaction::errors::TransactionExecutionError;
use thiserror::Error;

use crate::block_number::BlockNumber;
use crate::error::DatabaseError;

#[derive(Debug, Error)]
pub enum Error {
    /// `GenerateReplayWork` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    GenerateReplayWork(anyhow::Error),

    /// `ReplayBlocks` is used to encapsulate errors of type [`anyhow::Error`]
    /// which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    ReplayBlocks(anyhow::Error),

    /// `ExecuteBlock` is used to encapsulate errors of type [`anyhow::Error`]
    /// which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    ExecuteBlock(anyhow::Error),

    /// `DatabaseAccess` is used to report [`crate::error::DatabaseError`] when
    /// database functions are called in the module [`crate::runner`].
    #[error(transparent)]
    DatabaseAccess(#[from] DatabaseError),

    /// `InsufficientBlocks` is triggered when the most recent block in the
    /// database is less than the starting block of the replay. For obvious
    /// reasons the tool can't continue.
    #[error(
        "Most recent block found in the databse is {last_block}. Exiting because less than \
         start_block {start_block}"
    )]
    InsufficientBlocks {
        last_block: BlockNumber,
        start_block: BlockNumber,
    },

    /// `IntegerTooLarge` variant is triggered when casting from `u64` to
    /// `usize` returns an error.
    #[error(transparent)]
    IntegerTooLarge(#[from] TryFromIntError),

    /// `FileIO` variant is for errors reported when saving transaction traces
    /// to JSON file.
    #[error(transparent)]
    FileIO(#[from] std::io::Error),

    /// `Serde` variant is for errors reported by the crate [`serde_json`].
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// `Execution` variant is for errors reported during transaction replay
    /// from [`blockifier`].
    #[error(transparent)]
    Execution(#[from] TransactionExecutionError),

    /// `State` variant is for errors reported when calling the function
    /// [`blockifier::blockifier::block::pre_process_block`].
    #[error(transparent)]
    State(#[from] StateError),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error during block replay: {0:?}")]
    Unknown(String),
}
