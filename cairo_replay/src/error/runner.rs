//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use pathfinder_executor::TransactionExecutionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `Serde` variant is for errors reported by the crate `serde_json`.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// `CairoLangRunner` variant is for errors reported by the crate
    /// `cairo_lang_runner`.
    #[error(transparent)]
    CairoLangRunner(#[from] CairoError),

    /// `CairoLangSierra` variant is for errors reported by the crate
    /// `cairo_lang_sierra`.
    #[error(transparent)]
    CairoLangSierra(#[from] Box<ProgramRegistryError>),

    /// `CairoLangSierraToCasm` is for errors reported by the crate
    /// `cairo_lang_sierra_to_casm`.
    #[error(transparent)]
    CairoLangSierraToCasm(#[from] Box<CompilationError>),

    /// `PathfinderExecutor` is for errors reported by the crate
    /// `pathfinder_executor`.
    #[error(transparent)]
    PathfinderExecutor(#[from] TransactionExecutionError),

    /// `GetContractClassAtBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `cairo_replay::runner::get_contract_class_at_block`.
    #[error(transparent)]
    GetContractClassAtBlock(anyhow::Error),

    /// `GenerateReplayWork` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `cairo_replay::generate_replay_work`.
    #[error(transparent)]
    GenerateReplayWork(anyhow::Error),

    /// `ReplayBlocks` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `cairo_replay::replay_blocks`.
    #[error(transparent)]
    ReplayBlocks(anyhow::Error),

    /// `ExecuteBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `cairo_replay::execute_block`.
    #[error(transparent)]
    ExecuteBlock(anyhow::Error),

    /// `GetChainId` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `cairo_replay::get_chain_id`.
    #[error(transparent)]
    GetChainId(anyhow::Error),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("error during block replay")]
    Unknown(String),
}
