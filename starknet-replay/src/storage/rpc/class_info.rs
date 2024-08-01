//! This module is responsible for constructing
//! [`blockifier::execution::contract_class::ClassInfo`] struct which is needed
//! to convert transactions is the format needed by [`blockifier`] for the
//! replay.

use blockifier::execution::contract_class::ClassInfo;
use starknet::core::types::ContractClass;
use starknet_api::core::ClassHash;
use starknet_api::transaction::{DeclareTransaction, Transaction};

use super::contract_class::decompress_sierra;
use crate::block_number::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::rpc::contract_class::decompress_casm;
use crate::storage::Storage;

/// This internal function returns the
/// [`blockifier::execution::contract_class::ClassInfo`] for both Sierra and
/// CASM contracts.
///
/// # Arguments
///
/// - `storage`: The struct to query the Starknet blockchain.
/// - `block_number`: The block number replayed.
/// - `class_hash`: The class hash.
///
/// # Errors
///
/// Returns [`Err`] if the RPC call fails or the data is not valid to create a
/// [`blockifier::execution::contract_class::ClassInfo`].
fn internal<T>(
    storage: &T,
    block_number: BlockNumber,
    class_hash: ClassHash,
) -> Result<ClassInfo, DatabaseError>
where
    T: Storage + Sync + Send,
{
    let replay_class_hash = ReplayClassHash {
        block_number,
        class_hash,
    };
    let class_definition = storage.get_contract_class_at_block(&replay_class_hash)?;
    let class_info = match class_definition {
        ContractClass::Sierra(flattened_sierra_cc) => {
            let sierra_program_length = flattened_sierra_cc.sierra_program.len();
            let abi_length = flattened_sierra_cc.abi.len();
            let contract_class = decompress_sierra(flattened_sierra_cc)?;
            ClassInfo::new(&contract_class, sierra_program_length, abi_length)?
        }
        ContractClass::Legacy(legacy_class) => {
            let contract_class = decompress_casm(legacy_class)?;
            ClassInfo::new(&contract_class, 0, 0)?
        }
    };
    Ok(class_info)
}

/// This the public function which returns the
/// [`blockifier::execution::contract_class::ClassInfo`] for both Sierra and
/// CASM contracts.
///
///
/// # Arguments
///
/// - `storage`: The struct to query the Starknet blockchain.
/// - `block_number`: The block number replayed.
/// - `tx`: The transaction to replay.
///
/// # Errors
///
/// Returns [`Err`] if the call to the internal function fails.
pub fn generate_class_info<T>(
    storage: &T,
    block_number: BlockNumber,
    tx: &Transaction,
) -> Result<Option<ClassInfo>, DatabaseError>
where
    T: Storage + Sync + Send,
{
    match tx {
        Transaction::Declare(tx) => match tx {
            DeclareTransaction::V0(tx) => {
                let class_hash = tx.class_hash;
                let class_info = internal(storage, block_number, class_hash)?;
                Ok(Some(class_info))
            }
            DeclareTransaction::V1(tx) => {
                let class_hash = tx.class_hash;
                let class_info = internal(storage, block_number, class_hash)?;
                Ok(Some(class_info))
            }
            DeclareTransaction::V2(tx) => {
                let class_hash = tx.class_hash;
                let class_info = internal(storage, block_number, class_hash)?;
                Ok(Some(class_info))
            }
            DeclareTransaction::V3(tx) => {
                let class_hash = tx.class_hash;
                let class_info = internal(storage, block_number, class_hash)?;
                Ok(Some(class_info))
            }
        },
        _ => Ok(None),
    }
}
