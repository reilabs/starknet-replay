//! This module uses the Starknet RPC protocol to query the data required to
//! replay transactions from the Starknet blockchain.

#![allow(clippy::module_name_repetitions)] // Added because of `ClassInfo`

use std::collections::BTreeMap;
use std::num::NonZeroU128;
use std::path::PathBuf;

use blockifier::blockifier::block::{pre_process_block, BlockInfo, BlockNumberHashPair, GasPrices};
use blockifier::bouncer::BouncerConfig;
use blockifier::context::{BlockContext, ChainInfo};
use blockifier::state::cached_state::CachedState;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
use blockifier::transaction::transaction_types::TransactionType;
use blockifier::transaction::transactions::ExecutableTransaction;
use blockifier::versioned_constants::VersionedConstants;
use once_cell::sync::Lazy;
use rpc_client::RpcClient;
use starknet_api::block::{BlockHeader, StarknetVersion};
use starknet_api::core::{ClassHash, ContractAddress, PatriciaKey};
use starknet_api::data_availability::L1DataAvailabilityMode;
use starknet_api::transaction::{Transaction, TransactionExecutionStatus, TransactionReceipt};
use starknet_api::{contract_address, felt, patricia_key};
use starknet_core::types::{
    ContractClass,
    ContractStorageDiffItem,
    DeclaredClassItem,
    DeployedContractItem,
    Felt,
    NonceUpdate,
    ReplacedClassItem,
    StateDiff,
    StorageEntry,
};
use tracing::{error, info, warn};
use url::Url;

use self::visited_pcs::VisitedPcsRaw;
use crate::block_number::BlockNumber;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::{ReplayClassHash, TransactionOutput, VisitedPcs};
use crate::runner::replay_state_reader::ReplayStateReader;
use crate::runner::report::write_to_file;
use crate::storage::Storage as ReplayStorage;

pub mod class_info;
pub mod contract_class;
pub mod receipt;
pub mod rpc_client;
pub mod transaction;
pub mod visited_pcs;

/// These versioned constants are needed to replay transactions executed with
/// older Starknet versions.
/// This is for Starknet 0.13.1.0
static VERSIONED_CONSTANTS_13_0: Lazy<VersionedConstants> = Lazy::new(|| {
    // I am not using relative path to avoid IoError 36 "File name too long"
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/versioned_constants_13_0.json"
    )))
    .expect("Versioned constants JSON file is malformed")
});

/// This is for Starknet 0.13.1.1
static VERSIONED_CONSTANTS_13_1: Lazy<VersionedConstants> = Lazy::new(|| {
    // I am not using relative path to avoid IoError 36 "File name too long"
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/versioned_constants_13_1.json"
    )))
    .expect("Versioned constants JSON file is malformed")
});

/// This structure partially implements a Starknet RPC client.
///
/// The RPC calls included are those needed to replay transactions.
/// Clone is not derived because it's not supported by Client.
#[allow(dead_code)]
pub struct RpcStorage {
    /// The endpoint of the Starknet RPC Node.
    ///
    /// Unused but kept for reference.
    rpc_client: RpcClient,
}
impl RpcStorage {
    /// Constructs a new `RpcStorage`.
    ///
    /// # Arguments
    ///
    /// - `endpoint`: The Url of the Starknet RPC node.
    #[must_use]
    pub fn new(endpoint: Url) -> Self {
        let rpc_client = RpcClient::new(endpoint);
        RpcStorage { rpc_client }
    }

    /// Constructs the [`blockifier::context::ChainInfo`] struct for the
    /// replayer.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request of chain id fails.
    fn chain_info(&self) -> Result<ChainInfo, DatabaseError> {
        // NOTE: these are the same for _all_ networks
        let eth_fee_token_address =
            contract_address!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
        let strk_fee_token_address =
            contract_address!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

        // TODO: Allow different chains.
        let chain_id = self.rpc_client.starknet_get_chain_id()?;

        Ok(ChainInfo {
            chain_id,
            fee_token_addresses: blockifier::context::FeeTokenAddresses {
                strk_fee_token_address,
                eth_fee_token_address,
            },
        })
    }

    /// This function constructs the
    /// [`blockifier::blockifier::block::BlockInfo`] to replay transactions
    /// with blockifier.
    ///
    /// # Arguments
    ///
    /// - `header`: the block header of the block replay.
    /// - `allow_use_kzg_data`: use KZG commitments.
    fn block_info(header: &BlockHeader, allow_use_kzg_data: bool) -> BlockInfo {
        let price_one: NonZeroU128 = NonZeroU128::MIN;
        BlockInfo {
            block_number: header.block_number,
            block_timestamp: header.timestamp,
            sequencer_address: header.sequencer.0,
            gas_prices: GasPrices {
                // Bad API design - the genesis block has 0 gas price, but
                // blockifier doesn't allow for it. This isn't critical for
                // consensus, so we just use 1.
                eth_l1_gas_price: NonZeroU128::new(header.l1_gas_price.price_in_wei.0)
                    .unwrap_or(price_one),
                // Bad API design - the genesis block has 0 gas price, but
                // blockifier doesn't allow for it. This isn't critical for
                // consensus, so we just use 1.
                strk_l1_gas_price: NonZeroU128::new(header.l1_gas_price.price_in_fri.0)
                    .unwrap_or(price_one),
                // Bad API design - pre-v0.13.1 blocks have 0 data gas price, but
                // blockifier doesn't allow for it. This value is ignored for those
                // transactions.
                eth_l1_data_gas_price: NonZeroU128::new(header.l1_data_gas_price.price_in_wei.0)
                    .unwrap_or(price_one),
                // Bad API design - pre-v0.13.1 blocks have 0 data gas price, but
                // blockifier doesn't allow for it. This value is ignored for those
                // transactions.
                strk_l1_data_gas_price: NonZeroU128::new(header.l1_data_gas_price.price_in_fri.0)
                    .unwrap_or(price_one),
            },
            use_kzg_da: allow_use_kzg_data && header.l1_da_mode == L1DataAvailabilityMode::Blob,
        }
    }

    /// Returns a reference to
    /// [`blockifier::versioned_constants::VersionedConstants`] for the
    /// starknet version.
    ///
    /// # Arguments
    ///
    /// - `starknet_version`: the starknet version of the block to replay.
    fn versioned_constants(starknet_version: &StarknetVersion) -> &'static VersionedConstants {
        if starknet_version < &StarknetVersion("0.13.1.0".to_string()) {
            &VERSIONED_CONSTANTS_13_0
        } else if starknet_version < &StarknetVersion("0.13.1.1".to_string()) {
            &VERSIONED_CONSTANTS_13_1
        } else {
            VersionedConstants::latest_constants()
        }
    }

    /// Returns the [`starknet_api::core::ClassHash`] of a Declare transaction
    /// of a Cairo0 contract.
    ///
    /// Returns `None` if it's not a Declare transaction or the contract is a
    /// Sierra contract.
    ///
    /// # Arguments
    ///
    /// - `transaction`: the transaction object.
    fn transaction_declared_deprecated_class(
        transaction: &blockifier::transaction::transaction_execution::Transaction,
    ) -> Option<ClassHash> {
        match transaction {
            BlockifierTransaction::AccountTransaction(
                blockifier::transaction::account_transaction::AccountTransaction::Declare(tx),
            ) => match tx.tx() {
                starknet_api::transaction::DeclareTransaction::V0(_)
                | starknet_api::transaction::DeclareTransaction::V1(_) => Some(tx.class_hash()),
                starknet_api::transaction::DeclareTransaction::V2(_)
                | starknet_api::transaction::DeclareTransaction::V3(_) => None,
            },
            _ => None,
        }
    }

    /// Returns the blockchain state change after a transaction execution.
    ///
    /// # Arguments
    ///
    /// - `state`: the blockchain state object.
    /// - `old_declared_contract`: new Cairo0 contract being declared, otherwise
    ///   `None`.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the computation of state changes fails.
    fn to_state_diff<S: StateReader, V: blockifier::state::visited_pcs::VisitedPcs>(
        state: &mut CachedState<S, V>,
        old_declared_contract: Option<ClassHash>,
    ) -> Result<StateDiff, StateError> {
        let state_diff = state.to_state_diff()?;

        let mut deployed_contracts = Vec::new();
        let mut replaced_classes = Vec::new();

        // We need to check the previous class hash for a contract to decide if it's a
        // deployed contract or a replaced class.
        for (address, class_hash) in state_diff.class_hashes {
            let previous_class_hash = state.state.get_class_hash_at(address)?;

            if previous_class_hash.0 == Felt::ZERO {
                deployed_contracts.push(DeployedContractItem {
                    address: *address.0.key(),
                    class_hash: class_hash.0,
                });
            } else {
                replaced_classes.push(ReplacedClassItem {
                    contract_address: *address.0.key(),
                    class_hash: class_hash.0,
                });
            }
        }

        let mut diffs: BTreeMap<Felt, Vec<StorageEntry>> = BTreeMap::new();
        state_diff
            .storage
            .into_iter()
            .for_each(|((address, storage_key), storage_value)| {
                let storage_entry = StorageEntry {
                    key: storage_key.into(),
                    value: storage_value,
                };
                diffs.entry(address.into()).or_default().push(storage_entry);
            });

        let storage_diffs: Vec<ContractStorageDiffItem> = diffs
            .into_iter()
            .map(|(address, storage_entries)| ContractStorageDiffItem {
                address,
                storage_entries,
            })
            .collect();

        Ok(StateDiff {
            storage_diffs,
            deployed_contracts,
            // This info is not present in the state diff, so we need to pass it separately.
            deprecated_declared_classes: old_declared_contract
                .into_iter()
                .map(|class_hash| class_hash.0)
                .collect(),
            declared_classes: state_diff
                .compiled_class_hashes
                .into_iter()
                .map(|(class_hash, compiled_class_hash)| DeclaredClassItem {
                    class_hash: class_hash.0,
                    compiled_class_hash: compiled_class_hash.0,
                })
                .collect(),
            nonces: state_diff
                .nonces
                .into_iter()
                .map(|(address, nonce)| NonceUpdate {
                    contract_address: *address.0.key(),
                    nonce: nonce.0,
                })
                .collect(),
            replaced_classes,
        })
    }

    /// Returns the
    /// [`blockifier::transaction::transaction_types::TransactionType`] of a
    /// transaction.
    ///
    /// # Arguments
    ///
    /// - `transaction`: the transaction object.
    fn transaction_type(
        transaction: &blockifier::transaction::transaction_execution::Transaction,
    ) -> TransactionType {
        match transaction {
            BlockifierTransaction::AccountTransaction(tx) => match tx {
                blockifier::transaction::account_transaction::AccountTransaction::Declare(_) => {
                    TransactionType::Declare
                }
                blockifier::transaction::account_transaction::AccountTransaction::DeployAccount(
                    _,
                ) => TransactionType::DeployAccount,
                blockifier::transaction::account_transaction::AccountTransaction::Invoke(_) => {
                    TransactionType::InvokeFunction
                }
            },
            BlockifierTransaction::L1HandlerTransaction(_) => TransactionType::L1Handler,
        }
    }
}
impl ReplayStorage for RpcStorage {
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let block_number = self.rpc_client.starknet_block_number()?;
        Ok(block_number)
    }

    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        let contract_class = self.rpc_client.starknet_get_class(replay_class_hash)?;
        Ok(contract_class)
    }

    fn get_block_header(&self, block_number: BlockNumber) -> Result<BlockHeader, DatabaseError> {
        let block_header = self
            .rpc_client
            .starknet_get_block_with_tx_hashes(&block_number)?;
        Ok(block_header)
    }

    fn get_transactions_and_receipts_for_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<(Vec<Transaction>, Vec<TransactionReceipt>), DatabaseError> {
        let transactions = self
            .rpc_client
            .starknet_get_block_with_receipts(&block_number)?;
        Ok(transactions)
    }

    #[allow(clippy::too_many_lines)] // Added because it can't be meaningfully split further in smaller blocks.
    fn execute_block(
        &self,
        work: &ReplayBlock,
        trace_out: &Option<PathBuf>,
    ) -> Result<Vec<TransactionOutput>, RunnerError> {
        let block_number = BlockNumber::new(work.header.block_number.0);

        let mut transactions: Vec<BlockifierTransaction> =
            Vec::with_capacity(work.transactions.len());
        for (transaction, receipt) in work.transactions.iter().zip(work.receipts.iter()) {
            let tx = transaction;
            let tx_hash = receipt.transaction_hash;
            let class_info = class_info::generate_class_info(self, block_number, tx)?;

            let paid_fee_on_l1 = match tx {
                Transaction::L1Handler(_) => {
                    Some(starknet_api::transaction::Fee(1_000_000_000_000))
                }
                _ => None,
            };

            let deployed_contract_address = match &receipt.output {
                starknet_api::transaction::TransactionOutput::DeployAccount(receipt) => {
                    Some(receipt.contract_address)
                }
                _ => None,
            };

            let only_query = false;
            let transaction = BlockifierTransaction::from_api(
                tx.to_owned(),
                tx_hash,
                class_info,
                paid_fee_on_l1,
                deployed_contract_address,
                only_query,
            )?;

            transactions.push(transaction);
        }

        // Transactions are replayed with the call to `ExecutableTransaction::execute`.
        // When simulating transactions, the storage layer should match the data of the
        // parent block (i.e. before the transaction is executed)
        let block_number_minus_one = BlockNumber::new(work.header.block_number.0 - 1);
        let state_reader = ReplayStateReader::new(&self.rpc_client, block_number_minus_one);
        let charge_fee = true;
        let validate = true;
        let allow_use_kzg_data = true;
        let chain_info = self.chain_info()?;
        let block_info = Self::block_info(&work.header, allow_use_kzg_data);
        let old_block_number_and_hash = if work.header.block_number.0 >= 10 {
            let block_number_whose_hash_becomes_available =
                BlockNumber::new(work.header.block_number.0 - 10);
            let block_hash = self
                .rpc_client
                .starknet_get_block_with_tx_hashes(&block_number_whose_hash_becomes_available)?
                .block_hash;

            Some(BlockNumberHashPair::new(
                block_number_whose_hash_becomes_available.get(),
                block_hash.0,
            ))
        } else {
            None
        };
        let starknet_version = work.header.starknet_version.clone();
        let versioned_constants = Self::versioned_constants(&starknet_version);
        let mut state: CachedState<_, VisitedPcsRaw> = CachedState::new(state_reader);
        let block_context = BlockContext::new(
            block_info,
            chain_info,
            versioned_constants.clone(),
            BouncerConfig::max(),
        );
        pre_process_block(
            &mut state,
            old_block_number_and_hash,
            work.header.block_number,
        )?;

        let mut transaction_result: Vec<_> = Vec::with_capacity(transactions.len());
        for (idx, transaction) in transactions.iter().enumerate() {
            let tx_type = Self::transaction_type(transaction);
            let transaction_declared_deprecated_class_hash =
                Self::transaction_declared_deprecated_class(transaction);
            let mut tx_state = CachedState::<_, _>::create_transactional(&mut state);
            // No fee is being calculated.
            let tx_info = transaction.execute(&mut tx_state, &block_context, charge_fee, validate);
            let state_diff =
                Self::to_state_diff(&mut tx_state, transaction_declared_deprecated_class_hash)?;
            tx_state.commit();
            // TODO: Cache the storage changes for faster storage access.
            match tx_info {
                // TODO: This clone should be avoided for efficiency.
                Ok(tx_info) => {
                    let receipt = &work.receipts[idx];
                    let tx_hash = receipt.transaction_hash;
                    match (&tx_info.revert_error, receipt.output.execution_status()) {
                        (None, TransactionExecutionStatus::Reverted(revert_error)) => {
                            let revert_error = &revert_error.revert_reason;
                            warn!(
                                "Transaction replay succeeded, expected reverted. {tx_hash:?} | \
                                 {revert_error}"
                            );
                        }
                        (Some(revert_error), TransactionExecutionStatus::Succeeded) => {
                            warn!(
                                "Transaction replay reverted, expected succeess. {tx_hash:?} | \
                                 {revert_error}"
                            );
                        }
                        (Some(_), TransactionExecutionStatus::Reverted(_))
                        | (None, TransactionExecutionStatus::Succeeded) => (),
                    };

                    let visited_pcs: VisitedPcs = state
                        .visited_pcs
                        .clone()
                        .0
                        .into_iter()
                        .map(|(class_hash, pcs)| {
                            let replay_class_hash = ReplayClassHash {
                                block_number,
                                class_hash,
                            };
                            (replay_class_hash, pcs.into_iter().collect())
                        })
                        .collect();
                    if let Some(filename) = trace_out {
                        write_to_file(filename, &tx_info, tx_type, Some(state_diff))?;
                        info!("Saved transaction trace block {block_number}");
                    }
                    transaction_result.push((tx_info, visited_pcs));
                }
                Err(err) => {
                    let receipt = &work.receipts[idx];
                    let tx_hash = receipt.transaction_hash;
                    error!(
                        "Interrupting {block_number} block replay. Transaction {tx_hash:?} \
                         exception {err:?}"
                    );
                    return Err(RunnerError::Unknown(err.to_string()));
                }
            }
        }
        Ok(transaction_result)
    }
}

#[cfg(test)]
mod tests {

    use starknet_api::core::{ChainId, Nonce};
    use starknet_api::felt;
    use starknet_api::hash::StarkHash;
    use starknet_api::state::StorageKey;

    use super::*;

    fn build_rpc_storage() -> RpcStorage {
        let endpoint: Url =
            Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
        RpcStorage::new(endpoint)
    }

    #[test]
    fn test_block_number() {
        let rpc_storage = build_rpc_storage();
        let block_number = rpc_storage.rpc_client.starknet_block_number().unwrap();
        // Mainnet block is more than 600k at the moment.
        assert!(block_number.get() > 600_000);
    }

    #[test]
    fn test_get_class() {
        let rpc_storage = build_rpc_storage();
        let class_hash: StarkHash =
            Felt::from_hex("0x029927c8af6bccf3f6fda035981e765a7bdbf18a2dc0d630494f8758aa908e2b")
                .unwrap();
        let replay_class_hash = ReplayClassHash {
            block_number: BlockNumber::new(632_917),
            class_hash: ClassHash(class_hash),
        };
        let contract_class = rpc_storage
            .rpc_client
            .starknet_get_class(&replay_class_hash)
            .unwrap();

        match contract_class {
            ContractClass::Sierra(contract_class) => {
                assert_eq!(contract_class.entry_points_by_type.l1_handler.len(), 0);
                assert_eq!(contract_class.entry_points_by_type.constructor.len(), 1);
                assert_eq!(contract_class.entry_points_by_type.external.len(), 32);
                assert_eq!(
                    contract_class.sierra_program.get(16).unwrap().to_owned(),
                    Felt::from_hex(
                        "0x02ee1e2b1b89f8c495f200e4956278a4d47395fe262f27b52e5865c9524c08c3"
                    )
                    .unwrap()
                );
            }
            ContractClass::Legacy(_) => panic!("Test failed, contract must be a Sierra contract."),
        }
    }

    #[test]
    fn test_get_block_with_tx_hashes() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        let block_header = rpc_storage
            .rpc_client
            .starknet_get_block_with_tx_hashes(&block_number)
            .unwrap();
        assert_eq!(block_header.timestamp.0, 1_713_168_820);
    }

    #[test]
    fn test_get_block_with_receipts() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        rpc_storage
            .rpc_client
            .starknet_get_block_with_receipts(&block_number)
            .unwrap();
    }

    #[test]
    fn test_get_nonce() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        let contract_address: ContractAddress =
            contract_address!("0x0710ce97d91835e049e5e76fbcb594065405744cf057b5b5f553282108983c53");
        let nonce = rpc_storage
            .rpc_client
            .starknet_get_nonce(&block_number, &contract_address)
            .unwrap();

        let nonce_expected: StarkHash = Felt::from_hex("0x4").unwrap();
        let nonce_expected = Nonce(nonce_expected);

        assert_eq!(nonce, nonce_expected);
    }

    #[test]
    fn test_get_class_hash_at() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_916);
        let contract_address: ContractAddress =
            contract_address!("0x0710ce97d91835e049e5e76fbcb594065405744cf057b5b5f553282108983c53");
        let class_hash = rpc_storage
            .rpc_client
            .starknet_get_class_hash_at(&block_number, &contract_address)
            .unwrap();

        let class_hash_expected: StarkHash =
            Felt::from_hex("0x3530cc4759d78042f1b543bf797f5f3d647cde0388c33734cf91b7f7b9314a9")
                .unwrap();
        let class_hash_expected = ClassHash(class_hash_expected);

        assert_eq!(class_hash, class_hash_expected);
    }

    #[test]
    fn test_get_class_hash_at_contract_just_created() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        let contract_address: ContractAddress =
            contract_address!("0x568f8c3532c549ad331a48d86d93b20064c3c16ac6bf396f041107cc6078707");
        let class_hash = rpc_storage
            .rpc_client
            .starknet_get_class_hash_at(&block_number, &contract_address)
            .unwrap();

        let class_hash_expected: StarkHash =
            Felt::from_hex("0x01a736d6ed154502257f02b1ccdf4d9d1089f80811cd6acad48e6b6a9d1f2003")
                .unwrap();
        let class_hash_expected = ClassHash(class_hash_expected);

        assert_eq!(class_hash, class_hash_expected);
    }

    #[test]
    fn test_get_storage_at() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        let contract_address: ContractAddress =
            contract_address!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
        let storage_key: StorageKey = StorageKey(patricia_key!(
            "0x06ccc0ef4c95ff7991520b6a5c45f8e688a4ae7e32bd108ec5392261a42b5306"
        ));
        let storage_value = rpc_storage
            .rpc_client
            .starknet_get_storage_at(&block_number, &contract_address, &storage_key)
            .unwrap();

        let storage_value_expected: Felt = Felt::from_hex("0x397de273516b4").unwrap();
        assert_eq!(storage_value, storage_value_expected);
    }

    #[test]
    fn test_get_chain_id() {
        let rpc_storage = build_rpc_storage();
        let main_chain = ChainId::from("SN_MAIN".to_string());
        let chain_id = rpc_storage.rpc_client.starknet_get_chain_id().unwrap();
        assert_eq!(chain_id, main_chain);
        assert_eq!(chain_id.as_hex(), main_chain.as_hex());
    }

    #[test]
    fn test_versioned_constants() {
        let starknet_version = StarknetVersion("0.13.0.0".to_string());
        let constants = RpcStorage::versioned_constants(&starknet_version);
        assert_eq!(constants.invoke_tx_max_n_steps, 3_000_000);

        let starknet_version = StarknetVersion("0.13.1.0".to_string());
        let constants = RpcStorage::versioned_constants(&starknet_version);
        assert_eq!(constants.invoke_tx_max_n_steps, 4_000_000);
    }
}
