//! This file contains the enum `Error` for all the errors returned by the
//! module `storage`.

use std::str::Utf8Error;

use blockifier::execution::errors::ContractClassError;
use cairo_lang_starknet_classes::casm_contract_class::StarknetSierraCompilationError;
use hex::FromHexError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `FileIO` variant is used for io errors.
    #[error(transparent)]
    FileIO(#[from] std::io::Error),

    /// `Serde` variant is used for errors reported by the crate [`serde_json`].
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    /// `Starknet` variant is used for errors reported by the crate
    /// [`starknet_api`]
    #[error(transparent)]
    Starknet(#[from] starknet_api::StarknetApiError),

    /// `DecodeHex` variant is used for errors reported by the crate [`hex`]
    /// used to convert from hex to ASCII string.
    #[error(transparent)]
    DecodeHex(#[from] FromHexError),

    /// `InvalidHex` variant is used for strings not matching a hex number.
    #[error("Chain id returned from RPC endpoint is not valid hex number.")]
    InvalidHex(),

    /// `DecodeBytes` variant is used for errors arising from converting a slice
    /// of bytes into a [`String`].
    #[error(transparent)]
    DecodeBytes(#[from] Utf8Error),

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

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error: {0:?}")]
    Unknown(String),
}
