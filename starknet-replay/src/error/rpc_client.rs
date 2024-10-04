//! This file contains the enum `Error` for all the errors returned by the
//! structure [`crate::storage::rpc::state::rpc_client::RpcClient`].

use std::str::Utf8Error;

use hex::FromHexError;
use starknet_providers::ProviderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// The `RpcResponse` variant is for errors generated when waiting for the
    /// RPC response.
    #[error(transparent)]
    RpcResponse(#[from] ProviderError),

    /// The `ParseInt` variant is used for errors generated when casting a
    /// string to an integer.
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),

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
}
