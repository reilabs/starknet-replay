//! This file contains the enum `Error` for all the errors returned by the
//! structure [`crate::storage::rpc::state::permanent_state::PermanentState`].

use thiserror::Error;

use crate::error::RpcClientError;

#[derive(Debug, Error)]
pub enum Error {
    /// The `RpcResponse` variant is for errors generated when waiting for the
    /// RPC response.
    #[error(transparent)]
    Rpc(#[from] RpcClientError),
}
