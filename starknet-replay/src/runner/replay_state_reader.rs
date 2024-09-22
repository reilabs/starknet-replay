//! This module implements [`blockifier::state::state_api::StateReader`] for use
//! in Starknet transaction replay. The functions to read the blockchain state
//! use the Starknet RPC protocol.

use blockifier::execution::contract_class::ContractClass as BlockifierContractClass;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::state::StorageKey;
use starknet_core::types::{ContractClass as StarknetContractClass, Felt};
use tracing::info;

use crate::block_number::BlockNumber;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::rpc::{contract_class, RpcStorage};

/// This structure is used by [`blockifier`] to access blockchain data during
/// transaction replay.
pub struct ReplayStateReader<'a> {
    /// The reference to [`crate::storage::rpc::RpcStorage`] to make RPC calls.
    storage: &'a RpcStorage,

    /// The block number used to query the state.
    block_number: BlockNumber,
}
impl ReplayStateReader<'_> {
    /// Constructs a new [`ReplayStateReader`] object.
    ///
    /// # Arguments
    ///
    /// - `storage`: The object exposing RPC calls to query the blockchain
    ///   state.
    /// - `block_number`: The block number at which state is read.
    #[must_use]
    pub fn new(storage: &RpcStorage, block_number: BlockNumber) -> ReplayStateReader<'_> {
        ReplayStateReader {
            storage,
            block_number,
        }
    }
}
impl StateReader for ReplayStateReader<'_> {
    fn get_storage_at(
        &self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<Felt> {
        let storage_value = self
            .storage
            .starknet_get_storage_at(&self.block_number, &contract_address, &key)
            .map_err(|_| {
                StateError::StateReadError("failed call to starknet_get_storage_at".to_string())
            })?;
        Ok(storage_value)
    }

    fn get_nonce_at(&self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self
            .storage
            .starknet_get_nonce(&self.block_number, &contract_address)
            .map_err(|_| {
                StateError::StateReadError("failed call to starknet_get_nonce".to_string())
            })?;
        Ok(nonce)
    }

    fn get_class_hash_at(&self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        let class_hash = self
            .storage
            .starknet_get_class_hash_at(&self.block_number, &contract_address)
            .map_err(|_| {
                StateError::StateReadError("failed call to starknet_get_class_hash_at".to_string())
            })?;
        Ok(class_hash)
    }

    fn get_compiled_contract_class(
        &self,
        class_hash: ClassHash,
    ) -> StateResult<BlockifierContractClass> {
        let replay_class_hash = ReplayClassHash {
            block_number: self.block_number,
            class_hash,
        };
        let contract_class = self
            .storage
            .starknet_get_class(&replay_class_hash)
            .map_err(|_| {
                StateError::StateReadError("failed call to starknet_get_class".to_string())
            })?;
        match contract_class {
            StarknetContractClass::Sierra(flattened_sierra_cc) => {
                let compiled_contract = contract_class::decompress_sierra(flattened_sierra_cc)
                    .map_err(|_| {
                        StateError::StateReadError(
                            "failed extraction of BlockifierContractClass".to_string(),
                        )
                    })?;
                Ok(compiled_contract)
            }
            StarknetContractClass::Legacy(flattened_casm_cc) => {
                let compiled_contract = contract_class::decompress_casm(flattened_casm_cc)
                    .map_err(|_| {
                        StateError::StateReadError(
                            "failed extraction of BlockifierContractClass".to_string(),
                        )
                    })?;
                Ok(compiled_contract)
            }
        }
    }

    fn get_compiled_class_hash(&self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        let replay_class_hash = ReplayClassHash {
            block_number: self.block_number,
            class_hash,
        };
        let contract_class = self
            .storage
            .starknet_get_class(&replay_class_hash)
            .map_err(|_| {
                StateError::StateReadError("failed call to starknet_get_class".to_string())
            })?;
        match contract_class {
            StarknetContractClass::Sierra(flattened_sierra_cc) => {
                let compiled_class_hash = contract_class::get_sierra_compiled_class_hash(
                    flattened_sierra_cc,
                )
                .map_err(|_| {
                    StateError::StateReadError(
                        "failed extraction of compiled class hash".to_string(),
                    )
                })?;
                info!(
                    "Compiled class hash for {:?} is {:?}",
                    class_hash, compiled_class_hash
                );
                Ok(CompiledClassHash(compiled_class_hash))
            }
            StarknetContractClass::Legacy(_) => Ok(CompiledClassHash(Felt::ZERO)),
        }
    }
}
