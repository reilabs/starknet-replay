use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Cairo(#[from] CairoError),

    #[error(transparent)]
    CairoLangSierra(#[from] Box<ProgramRegistryError>),

    #[error(transparent)]
    CairoLangSierraToCasm(#[from] Box<CompilationError>),

    #[error("error during block replay")]
    Unknown(String),
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error::Unknown(value.to_string())
    }
}
