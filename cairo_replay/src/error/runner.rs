//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use pathfinder_executor::TransactionExecutionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// For errors reported by the crate `serde_json`.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// For errors reported by the crate `cairo_lang_runner`.
    #[error(transparent)]
    CairoLangRunner(#[from] CairoError),

    /// For errors reported by the crate `cairo_lang_sierra`.
    #[error(transparent)]
    CairoLangSierra(#[from] Box<ProgramRegistryError>),

    /// For errors reported by the crate `cairo_lang_sierra_to_casm`.
    #[error(transparent)]
    CairoLangSierraToCasm(#[from] Box<CompilationError>),

    /// For errors reported by the crate `pathfinder_executor`.
    #[error(transparent)]
    PathfinderExecutor(#[from] TransactionExecutionError),

    /// For any other uncategorised error.
    #[error("error during block replay")]
    Unknown(String),
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error::Unknown(value.to_string())
    }
}
