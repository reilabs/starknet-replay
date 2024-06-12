//! This file contains the enum `Error` for all the errors returned by the
//! module `profiler`.

use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `CairoLangRunner` variant is for errors reported by the crate
    /// [`cairo_lang_runner`].
    #[error(transparent)]
    CairoLangRunner(#[from] CairoError),

    /// `CairoLangSierra` variant is for errors reported by the crate
    /// [`cairo_lang_sierra`].
    #[error(transparent)]
    CairoLangSierra(#[from] Box<ProgramRegistryError>),

    /// `CairoLangSierraToCasm` is for errors reported by the crate
    /// [`cairo_lang_sierra_to_casm`].
    #[error(transparent)]
    CairoLangSierraToCasm(#[from] Box<CompilationError>),

    /// `SierraStatementNotFound` is returned in function
    /// [`crate::profiler::SierraProfiler#method.collect_profiling_info`] when
    /// the index doesn't exist in the list of Sierra statements.
    #[error("Failed fetching Sierra statement index {0}. Can't continue profiling.")]
    SierraStatementNotFound(usize),

    /// `EmptyStatementList` is returned in function
    /// [`crate::profiler::SierraProfiler#method.collect_profiling_info`] when
    /// the list of Sierra statements is empty.
    #[error("The list of Sierra statements is empty. Can't continue profiling.")]
    EmptyStatementList,

    /// `EmptyProgramCounterList` is returned in function
    /// [`crate::profiler::SierraProfiler#method.collect_profiling_info`] when
    /// the list of program counters is empty.
    #[error("The list of visited program counters is empty. Can't continue profiling.")]
    EmptyProgramCounterList,

    /// `Save` variant is for errors reported when saving the result of libfuncs
    /// statistics to file.
    #[error(transparent)]
    Save(#[from] std::io::Error),

    /// `Serde` variant is for errors reported by the crate [`serde_json`].
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error during in profiler: {0:?}")]
    Unknown(String),
}
