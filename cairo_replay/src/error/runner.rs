use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("error while replaying block")]
    Unknown(String),
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error::Unknown(value.to_string())
    }
}

// TODO: should it be in a separate error variant?
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::Unknown(value.to_string())
    }
}

impl From<CairoError> for Error {
    fn from(value: CairoError) -> Self {
        Error::Unknown(value.to_string())
    }
}

impl From<Box<ProgramRegistryError>> for Error {
    fn from(value: Box<ProgramRegistryError>) -> Self {
        Error::Unknown(value.to_string())
    }
}

impl From<Box<CompilationError>> for Error {
    fn from(value: Box<CompilationError>) -> Self {
        Error::Unknown(value.to_string())
    }
}
