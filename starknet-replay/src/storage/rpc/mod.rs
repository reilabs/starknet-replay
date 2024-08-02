//! This module uses the Starknet RPC protocol to query the data required to
//! replay transactions from the Starknet blockchain.

#![allow(clippy::module_name_repetitions)] // Added because of `ClassInfo`

use std::num::NonZeroU128;
use std::path::Path;

use blockifier::block::{pre_process_block, BlockInfo, BlockNumberHashPair};
use blockifier::context::ChainInfo;
use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use blockifier::state::state_api::State;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use blockifier::versioned_constants::VersionedConstants;
use jsonrpc::minreq_http::MinreqHttpTransport;
use jsonrpc::{Client, Response};
use once_cell::sync::Lazy;
use serde_json::json;
use serde_json::value::{to_raw_value, RawValue};
use starknet::core::types::ContractClass;
use starknet_api::block::{BlockHeader, StarknetVersion};
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::data_availability::L1DataAvailabilityMode;
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::transaction::{Transaction, TransactionReceipt};
use starknet_api::{contract_address, patricia_key};
use url::Url;

use crate::block_number::BlockNumber;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::{ReplayClassHash, TransactionOutput, VisitedPcs};
use crate::runner::replay_state_reader::ReplayStateReader;
use crate::storage::rpc::receipt::deserialize_receipt_json;
use crate::storage::rpc::transaction::deserialize_transaction_json;
use crate::storage::Storage as ReplayStorage;

pub mod class_info;
pub mod contract_class;
pub mod receipt;
pub mod transaction;

/// These versioned constants are needed to replay transactions executed with
/// older Starknet versions.
/// This is for Starknet 0.13.1.0
static VERSIONED_CONSTANTS_13_0: Lazy<VersionedConstants> = Lazy::new(|| {
    VersionedConstants::try_from(Path::new(
        "../../../resources/versioned_constants_13_0.json",
    ))
    .expect("Versioned constants JSON file is malformed")
});

/// This is for Starknet 0.13.1.1
static VERSIONED_CONSTANTS_13_1: Lazy<VersionedConstants> = Lazy::new(|| {
    VersionedConstants::try_from(Path::new(
        "../../../resources/versioned_constants_13_1.json",
    ))
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
    endpoint: Url,

    /// The client field sends RPC calls.
    client: Client,
}
impl RpcStorage {
    /// Constructs a new `RpcStorage`.
    ///
    /// # Arguments
    ///
    /// - `endpoint`: The Url of the Starknet RPC node.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if [`jsonrpc::minreq_http::MinreqHttpTransport`] can't
    /// be created.
    pub fn new(endpoint: Url) -> Result<Self, DatabaseError> {
        let t = MinreqHttpTransport::builder()
            .url(endpoint.to_string().as_str())?
            .build();

        let client = Client::with_transport(t);
        Ok(RpcStorage { endpoint, client })
    }

    /// This function makes an RPC call and returns the response.
    ///
    /// # Arguments
    ///
    /// - `method`: The method of the RPC calls
    /// - `args`: The parameters of the RPC call. `None` if there are no
    ///   parameters.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    fn send_request(
        &self,
        method: &str,
        args: Option<&RawValue>,
    ) -> Result<Response, DatabaseError> {
        let request = self.client.build_request(method, args);
        tracing::info!("jsonrpc request {request:?}");
        Ok(self.client.send_request(request)?)
    }

    /// This function queries the number of the most recent Starknet block.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    pub fn starknet_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let response = self.send_request("starknet_blockNumber", None)?;
        let result: u64 = response.result()?;
        Ok(BlockNumber::new(result))
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
    ) -> Result<ContractClass, DatabaseError> {
        let class_hash = json!({ "block_id": { "block_number" : class_hash_at_block.block_number }, "class_hash": class_hash_at_block.class_hash });
        let args = to_raw_value(&class_hash)?;
        let response = self.send_request("starknet_getClass", Some(&*args))?;
        let result: ContractClass = response.result()?;
        Ok(result)
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
        let block_id = json!({ "block_id": { "block_number" : block_number } });
        let args = to_raw_value(&block_id)?;
        let response = self.send_request("starknet_getBlockWithTxHashes", Some(&*args))?;
        let mut result: serde_json::Value = response.result()?;
        // `state_root` is set to `0x0` because it's not provided from the JSON RPC
        // endpoint. It is not needed to replay transactions.
        result["state_root"] = "0x0".into();
        result["sequencer"] = result["sequencer_address"].clone();
        result
            .as_object_mut()
            .ok_or(DatabaseError::Unknown(
                "Failed to serialise block header as object.".to_string(),
            ))?
            .remove("sequencer_address");
        let block_header = serde_json::from_value(result.clone())?;
        Ok(block_header)
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
    ) -> Result<(Vec<Transaction>, Vec<TransactionReceipt>), DatabaseError> {
        let block_id = json!({ "block_id": { "block_number" : block_number } });
        let args = to_raw_value(&block_id)?;
        let response = self.send_request("starknet_getBlockWithReceipts", Some(&*args))?;
        let result: serde_json::Value = response.result()?;
        let txs = &result["transactions"];
        let Some(txs) = txs.as_array() else {
            return Ok((Vec::new(), Vec::new()));
        };

        let mut transactions = Vec::with_capacity(txs.len());
        let mut receipts = Vec::with_capacity(txs.len());
        for tx in txs {
            let transaction: Transaction = deserialize_transaction_json(&tx["transaction"])?;
            let receipt: TransactionReceipt = deserialize_receipt_json(&result, &tx["receipt"])?;

            transactions.push(transaction);
            receipts.push(receipt);
        }
        Ok((transactions, receipts))
    }

    /// This function queries the nonce of a contract.
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
    ) -> Result<Nonce, DatabaseError> {
        let parameters = json!({ "block_id": { "block_number" : block_number }, "contract_address": contract_address });
        let args = to_raw_value(&parameters)?;
        let response = self.send_request("starknet_getNonce", Some(&*args))?;
        let Ok(result): Result<String, _> = response.result() else {
            return Ok(Nonce(StarkFelt::ZERO));
        };
        let nonce: StarkHash = result.as_str().try_into()?;
        Ok(Nonce(nonce))
    }

    /// This function queries the class hash of a contract.
    ///
    /// Returns 0 if the class hash doesn't exist.
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
    ) -> Result<ClassHash, DatabaseError> {
        let parameters = json!({ "block_id": { "block_number" : block_number }, "contract_address": contract_address });
        let args = to_raw_value(&parameters)?;
        let response = self.send_request("starknet_getClassHashAt", Some(&*args))?;
        let Ok(result): Result<String, _> = response.result() else {
            return Ok(ClassHash(StarkFelt::ZERO));
        };
        let class_hash: StarkHash = result.as_str().try_into()?;
        Ok(ClassHash(class_hash))
    }

    /// This function queries the value of a storage key.
    ///
    /// Returns 0 if the storage key doesn't exist.
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
    ) -> Result<StarkFelt, DatabaseError> {
        let parameters = json!({ "block_id": { "block_number" : block_number }, "contract_address": contract_address, "key": key });
        let args = to_raw_value(&parameters)?;
        let response = self.send_request("starknet_getStorageAt", Some(&*args))?;
        let Ok(result): Result<String, _> = response.result() else {
            return Ok(StarkFelt::ZERO);
        };
        let storage_value: StarkFelt = result.as_str().try_into()?;
        Ok(storage_value)
    }

    /// This function queries the chain id of the RPC endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or decoding hex values of the chain
    /// id fails.
    pub fn starknet_get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        let response = self.send_request("starknet_chainId", None)?;
        let result: String = response.result()?;
        let result: Vec<&str> = result.split("0x").collect();
        let decoded_result = hex::decode(result.last().ok_or(DatabaseError::InvalidHex())?)?;
        let chain_id = std::str::from_utf8(&decoded_result)?;
        let chain_id = ChainId(chain_id.to_string());
        Ok(chain_id)
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
        let chain_id = self.starknet_get_chain_id()?;

        Ok(ChainInfo {
            chain_id,
            fee_token_addresses: blockifier::context::FeeTokenAddresses {
                strk_fee_token_address,
                eth_fee_token_address,
            },
        })
    }

    /// This function constructs the [`blockifier::block::BlockInfo`] to replay
    /// transactions with blockifier.
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
            gas_prices: blockifier::block::GasPrices {
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
}
impl ReplayStorage for RpcStorage {
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let block_number = self.starknet_block_number()?;
        Ok(block_number)
    }

    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        let contract_class = self.starknet_get_class(replay_class_hash)?;
        Ok(contract_class)
    }

    fn get_block_header(&self, block_number: BlockNumber) -> Result<BlockHeader, DatabaseError> {
        let block_header = self.starknet_get_block_with_tx_hashes(&block_number)?;
        Ok(block_header)
    }

    fn get_transactions_and_receipts_for_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<(Vec<Transaction>, Vec<TransactionReceipt>), DatabaseError> {
        let transactions = self.starknet_get_block_with_receipts(&block_number)?;
        Ok(transactions)
    }

    fn execute_block(&self, work: &ReplayBlock) -> Result<Vec<TransactionOutput>, RunnerError> {
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
        let state_reader = ReplayStateReader::new(self, block_number_minus_one);
        let charge_fee = true;
        let validate = true;
        let allow_use_kzg_data = true;
        let chain_info = self.chain_info()?;
        let block_info = Self::block_info(&work.header, allow_use_kzg_data);
        let old_block_number_and_hash = if work.header.block_number.0 >= 10 {
            let block_number_whose_hash_becomes_available =
                BlockNumber::new(work.header.block_number.0 - 10);
            let block_hash = self
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
        let cache_size = 16;
        let mut state = CachedState::new(state_reader, GlobalContractCache::new(cache_size));
        let block_context = pre_process_block(
            &mut state,
            old_block_number_and_hash,
            block_info,
            chain_info,
            versioned_constants.clone(),
        )?;

        let mut transaction_result: Vec<_> = Vec::with_capacity(transactions.len());
        for transaction in transactions {
            let mut tx_state = CachedState::<_>::create_transactional(&mut state);
            // No fee is being calculated.
            let tx_info = transaction.execute(&mut tx_state, &block_context, charge_fee, validate);
            tx_state.to_state_diff();
            tx_state.commit();
            // TODO: Cache the storage changes for faster storage access.
            match tx_info {
                // TODO: This clone should be avoided for efficiency.
                Ok(tx_info) => {
                    let visited_pcs: VisitedPcs = state
                        .visited_pcs
                        .clone()
                        .into_iter()
                        .map(|(class_hash, pcs)| {
                            let replay_class_hash = ReplayClassHash {
                                block_number,
                                class_hash,
                            };
                            (replay_class_hash, pcs)
                        })
                        .collect();
                    transaction_result.push((tx_info, visited_pcs));
                }
                Err(err) => {
                    tracing::info!("Transaction failed {err:?}");
                    return Err(RunnerError::Unknown(err.to_string()));
                }
            }
        }
        Ok(transaction_result)
    }
}

#[cfg(test)]
mod tests {

    use starknet::core::types::FieldElement;
    use starknet_api::hash::StarkHash;

    use super::*;

    fn build_rpc_storage() -> RpcStorage {
        let endpoint: Url =
            Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
        RpcStorage::new(endpoint).unwrap()
    }

    #[test]
    fn test_block_number() {
        let rpc_storage = build_rpc_storage();
        let block_number = rpc_storage.starknet_block_number().unwrap();
        // Mainnet block is more than 600k at the moment.
        assert!(block_number.get() > 600_000);
    }

    #[test]
    fn test_get_class() {
        let rpc_storage = build_rpc_storage();
        let class_hash: StarkHash =
            "0x029927c8af6bccf3f6fda035981e765a7bdbf18a2dc0d630494f8758aa908e2b"
                .try_into()
                .unwrap();
        let replay_class_hash = ReplayClassHash {
            block_number: BlockNumber::new(632_917),
            class_hash: ClassHash(class_hash),
        };
        let contract_class = rpc_storage.starknet_get_class(&replay_class_hash).unwrap();

        match contract_class {
            ContractClass::Sierra(contract_class) => {
                assert_eq!(contract_class.entry_points_by_type.l1_handler.len(), 0);
                assert_eq!(contract_class.entry_points_by_type.constructor.len(), 1);
                assert_eq!(contract_class.entry_points_by_type.external.len(), 32);
                assert_eq!(
                    contract_class.sierra_program.get(16).unwrap().to_owned(),
                    FieldElement::from_hex_be(
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
            .starknet_get_block_with_tx_hashes(&block_number)
            .unwrap();
        assert_eq!(block_header.timestamp.0, 1_713_168_820);
    }

    #[test]
    fn test_get_block_with_receipts() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632_917);
        rpc_storage
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
            .starknet_get_nonce(&block_number, &contract_address)
            .unwrap();

        let nonce_expected: StarkHash = "0x4".try_into().unwrap();
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
            .starknet_get_class_hash_at(&block_number, &contract_address)
            .unwrap();

        let class_hash_expected: StarkHash =
            "0x3530cc4759d78042f1b543bf797f5f3d647cde0388c33734cf91b7f7b9314a9"
                .try_into()
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
            .starknet_get_class_hash_at(&block_number, &contract_address)
            .unwrap();

        let class_hash_expected: StarkHash =
            "0x01a736d6ed154502257f02b1ccdf4d9d1089f80811cd6acad48e6b6a9d1f2003"
                .try_into()
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
            .starknet_get_storage_at(&block_number, &contract_address, &storage_key)
            .unwrap();

        let storage_value_expected: StarkFelt = "0x397de273516b4".try_into().unwrap();
        assert_eq!(storage_value, storage_value_expected);
    }

    #[test]
    fn test_get_chain_id() {
        let rpc_storage = build_rpc_storage();
        let main_chain = ChainId("SN_MAIN".to_string());
        let chain_id = rpc_storage.starknet_get_chain_id().unwrap();
        assert_eq!(chain_id, main_chain);
        assert_eq!(chain_id.as_hex(), main_chain.as_hex());
    }
}
