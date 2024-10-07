//! This file contains the enum `Error` for all the errors returned by the
//! module `storage`.

use blockifier::execution::errors::ContractClassError;
use cairo_lang_starknet_classes::casm_contract_class::StarknetSierraCompilationError;
use thiserror::Error;

use super::PermanentStateError;

#[derive(Debug, Error)]
pub enum Error {
    /// `FileIO` variant is used for io errors.
    #[error(transparent)]
    FileIO(#[from] std::io::Error),

    /// `Serde` variant is used for errors reported by the crate [`serde_json`].
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// `SierraCompiler` variant is for errors reported when compiling Sierra
    /// contracts to CASM.
    #[error(transparent)]
    SierraCompiler(#[from] StarknetSierraCompilationError),

    /// The `IntoInvalid` variant is for errors arising from the use of
    /// `try_into`.
    #[error("Error converting {0:?} into {1:?}")]
    IntoInvalid(String, String),

    /// The `ClassInfoInvalid` variant is for errors generated when
    /// constructing a [`blockifier::execution::contract_class::ClassInfo`]
    #[error(transparent)]
    ClassInfoInvalid(#[from] ContractClassError),

    /// The `PermanentState` variant is for errors generated from the
    /// [`crate::storage::rpc::state::permanent_state::PermanentState`] methods.
    #[error(transparent)]
    PermanentState(#[from] PermanentStateError),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error: {0:?}")]
    Unknown(String),
}
