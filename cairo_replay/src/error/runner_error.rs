use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RunnerError {
    #[error("error while replaying block")]
    Error(String),
}

impl From<anyhow::Error> for RunnerError {
    fn from(value: anyhow::Error) -> Self {
        RunnerError::Error(value.to_string())
    }
}

// TODO: should it be in a separate error variant?
impl From<serde_json::Error> for RunnerError {
    fn from(value: serde_json::Error) -> Self {
        RunnerError::Error(value.to_string())
    }
}

impl From<CairoError> for RunnerError {
    fn from(value: CairoError) -> Self {
        RunnerError::Error(value.to_string())
    }
}

impl From<Box<ProgramRegistryError>> for RunnerError {
    fn from(value: Box<ProgramRegistryError>) -> Self {
        RunnerError::Error(value.to_string())
    }
}

impl From<Box<CompilationError>> for RunnerError {
    fn from(value: Box<CompilationError>) -> Self {
        RunnerError::Error(value.to_string())
    }
}
