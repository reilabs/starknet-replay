//! This module is an interface to access the blockchain data. If the data is
//! not available locally, it is pulled using the RPC protocol.

use std::sync::RwLock;

use blockifier::state::cached_state::StateMaps;
use starknet_api::block::BlockHeader;
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::state::StorageKey;
use starknet_core::types::{ContractClass, Felt};
use starknet_providers::ProviderError;
use url::Url;

use super::rpc_client::RpcClient;
use crate::block_number::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::BlockWithReceipts;

/// This structure contains the blockchain state.
///
/// If data is not available locally, it sends an RPC request.
pub struct PermanentState {
    /// The object to send RPC requests
    rpc_client: RpcClient,

    /// The local blockchain data.
    state: RwLock<StateMaps>,

    /// When the variable is `true`, then the local state is updated with the
    /// `state_diff` of the blocks replayed.
    ///
    /// This is set `true` for serial replay of blocks to ensure consistency
    /// between the state at the end of block `x` and the state at the beginning
    /// of block `x+1`. When performing parallel replay, it must be set to
    /// `false` to avoid state corruption leading to replay failures.
    ///
    /// When `false`, the element `state` in this structure is kept empty to
    /// ensure all data is queried through the RPC request.
    read_from_state: bool,
}
impl PermanentState {
    /// Constructs a new `PermanentState` object.
    ///
    /// # Arguments
    ///
    /// - `endpoint`: the url of the RPC server.
    /// - `read_from_state`: when `true` it updates the local layer with the
    ///   state diff.
    #[must_use]
    pub fn new(endpoint: Url, read_from_state: bool) -> Self {
        let rpc_client = RpcClient::new(endpoint);
        let state = RwLock::new(StateMaps::default());
        PermanentState {
            rpc_client,
            state,
            read_from_state,
        }
    }

    /// Updates the local state with the data in the `state_diff`.
    ///
    /// When `read_from_state` is `false`, `state` is not updated.
    pub fn update(&self, state_diff: &StateMaps) {
        if self.read_from_state {
            self.state.write().unwrap().extend(state_diff);
        }
    }

    /// Returns the most recent block number.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the RPC request fails.
    pub fn starknet_block_number(&self) -> Result<BlockNumber, ProviderError> {
        self.rpc_client.starknet_block_number()
    }

    /// This function queries the contract class at a specific block.
    ///
    /// # Arguments
    ///
    /// - `class_hash_at_block`: class hash of the contract to be returned.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or the class hash doesn't exist.
    pub fn starknet_get_class(
        &self,
        class_hash_at_block: &ReplayClassHash,
    ) -> Result<ContractClass, ProviderError> {
        self.rpc_client.starknet_get_class(class_hash_at_block)
    }

    /// This function queries the block header.
    ///
    /// # Arguments
    ///
    /// - `block_number`: the block number to be queried.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or the block number doesn't exist.
    pub fn starknet_get_block_with_tx_hashes(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockHeader, DatabaseError> {
        self.rpc_client
            .starknet_get_block_with_tx_hashes(block_number)
    }

    /// This function queries the transactions and receipts in a block.
    ///
    /// # Arguments
    ///
    /// - `block_number`: the block number to be queried.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or the block number doesn't exist.
    pub fn starknet_get_block_with_receipts(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockWithReceipts, DatabaseError> {
        self.rpc_client
            .starknet_get_block_with_receipts(block_number)
    }

    /// This function queries the nonce of a contract.
    ///
    /// First it checks the local state, if the `nonce` is not found, it sends
    /// an RPC request.
    ///
    /// # Arguments
    ///
    /// - `block_number`: the block number at which to query the nonce.
    /// - `contract_address`: the address of the contract.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or the block number doesn't exist.
    pub fn starknet_get_nonce(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<Nonce, ProviderError> {
        match self.state.read().unwrap().nonces.get(contract_address) {
            Some(nonce) => Ok(*nonce),
            None => self
                .rpc_client
                .starknet_get_nonce(block_number, contract_address),
        }
    }

    /// This function queries the class hash of a contract.
    ///
    /// First it checks the local state, if the `nonce` is not found, it sends
    /// an RPC request. Returns 0 if the class hash doesn't exist.
    ///
    /// # Arguments
    ///
    /// - `block_number`: the block number at which to query the class hash.
    /// - `contract_address`: the address of the contract.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    pub fn starknet_get_class_hash_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<ClassHash, ProviderError> {
        match self
            .state
            .read()
            .unwrap()
            .class_hashes
            .get(contract_address)
        {
            Some(class_hash) => Ok(*class_hash),
            None => self
                .rpc_client
                .starknet_get_class_hash_at(block_number, contract_address),
        }
    }

    /// This function queries the value of a storage key.
    ///
    /// First it checks the local state, if the `nonce` is not found, it sends
    /// an RPC request. Returns 0 if the storage key doesn't exist.
    ///
    /// # Arguments
    ///
    /// - `block_number`: the block number at which to query the storage key.
    /// - `contract_address`: the address of the contract.
    /// - `key`: the storage key to query.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    pub fn starknet_get_storage_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
        key: &StorageKey,
    ) -> Result<Felt, ProviderError> {
        match self
            .state
            .read()
            .unwrap()
            .storage
            .get(&(*contract_address, *key))
        {
            Some(value) => Ok(*value),
            None => self
                .rpc_client
                .starknet_get_storage_at(block_number, contract_address, key),
        }
    }

    /// This function queries the chain id of the RPC endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or decoding hex values of the chain
    /// id fails.
    pub fn starknet_get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        self.rpc_client.starknet_get_chain_id()
    }
}
