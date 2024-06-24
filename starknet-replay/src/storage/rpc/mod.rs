use std::num::NonZeroU128;
use std::ops::Deref;

use blockifier::block::{pre_process_block, BlockInfo, BlockNumberHashPair};
use blockifier::context::ChainInfo;
use blockifier::execution::contract_class::{
    ClassInfo,
    ContractClass as BlockifierContractClass,
    ContractClassV1,
};
use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use blockifier::state::state_api::StateReader;
use blockifier::transaction::objects::TransactionExecutionInfo;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use blockifier::versioned_constants::VersionedConstants;
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use jsonrpc::minreq_http::MinreqHttpTransport;
use jsonrpc::{Client, Response};
use serde_json::json;
use serde_json::value::{to_raw_value, RawValue};
use starknet::core::types::ContractClass;
use starknet_api::block::{BlockHeader, StarknetVersion};
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::data_availability::L1DataAvailabilityMode;
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::transaction::{DeclareTransaction, Transaction, TransactionReceipt};
use starknet_api::{contract_address, patricia_key};
use url::Url;

use crate::block_number::BlockNumber;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::runner::replay_state_reader::ReplayStateReader;
use crate::storage::rpc::receipt_api::deserialize_receipt_json;
use crate::storage::rpc::transaction::deserialize_transaction_json;
use crate::storage::Storage as ReplayStorage;

pub mod contract_class;
pub mod receipt_api;
pub mod transaction;

#[allow(dead_code)]
pub struct RpcStorage {
    endpoint: Url,
    client: Client,
}
impl RpcStorage {
    pub fn new(endpoint: Url) -> Result<Self, DatabaseError> {
        let t = MinreqHttpTransport::builder()
            .url(endpoint.to_string().as_str())?
            .build();

        let client = Client::with_transport(t);
        Ok(RpcStorage { endpoint, client })
    }

    fn send_request(
        &self,
        method: &str,
        args: Option<&RawValue>,
    ) -> Result<Response, DatabaseError> {
        let request = self.client.build_request(method, args);
        tracing::info!("jsonrpc request {request:?}");
        //println!("jsonrpc request {request:?}");
        Ok(self.client.send_request(request)?)
    }

    pub fn starknet_block_number(&self) -> Result<u64, DatabaseError> {
        let response = self.send_request("starknet_blockNumber", None)?;
        let result: u64 = response.result()?;
        Ok(result)
    }

    pub fn starknet_get_class(
        &self,
        class_hash_at_block: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        let args = to_raw_value(class_hash_at_block).unwrap();
        let response = self.send_request("starknet_getClass", Some(args.deref()))?;
        let result: ContractClass = response.result()?;
        Ok(result)
    }

    pub fn starknet_get_block_with_tx_hashes(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockHeader, DatabaseError> {
        let block_id = json!({ "block_id": block_number });
        let args = to_raw_value(&block_id).unwrap();
        let response = self.send_request("starknet_getBlockWithTxHashes", Some(args.deref()))?;
        let mut result: serde_json::Value = response.result()?;
        // Setting `state_root` to `0x0` because it's not provided from the JSON RPC
        // endpoint. It shouldn't be needed to replay transactions.
        result["state_root"] = "0x0".try_into().unwrap();
        result["sequencer"] = result["sequencer_address"].clone();
        result.as_object_mut().unwrap().remove("sequencer_address");
        let block_header = serde_json::from_value(result.clone()).unwrap();
        Ok(block_header)
    }

    pub fn starknet_get_block_with_receipts(
        &self,
        block_number: &BlockNumber,
    ) -> Result<(Vec<Transaction>, Vec<TransactionReceipt>), DatabaseError> {
        let block_id = json!({ "block_id": block_number });
        let args = to_raw_value(&block_id).unwrap();
        let response = self.send_request("starknet_getBlockWithReceipts", Some(args.deref()))?;
        let result: serde_json::Value = response.result()?;
        let txs = &result["transactions"];
        let txs = txs.as_array().unwrap();

        let mut transactions = Vec::with_capacity(txs.len());
        let mut receipts = Vec::with_capacity(txs.len());
        for tx in txs {
            let transaction: Transaction =
                deserialize_transaction_json(&tx["transaction"]).unwrap();
            let receipt: TransactionReceipt =
                deserialize_receipt_json(&result, &tx["receipt"]).unwrap();

            transactions.push(transaction);
            receipts.push(receipt);
        }
        Ok((transactions, receipts))
    }

    pub fn starknet_get_nonce(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<Nonce, DatabaseError> {
        let parameters = json!({ "block_id": block_number, "contract_address": contract_address });
        let args = to_raw_value(&parameters).unwrap();
        let response = self.send_request("starknet_getNonce", Some(args.deref()))?;
        let Ok(result): Result<String, _> = response.result() else {
            return Ok(Nonce(StarkFelt::ZERO.into()));
        };
        let nonce: StarkHash = result.as_str().try_into().unwrap();
        Ok(Nonce(nonce))
    }

    pub fn starknet_get_class_hash_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<ClassHash, DatabaseError> {
        let parameters = json!({ "block_id": block_number, "contract_address": contract_address });
        let args = to_raw_value(&parameters).unwrap();
        let response = self.send_request("starknet_getClassHashAt", Some(args.deref()))?;
        let Ok(result): Result<String, _> = response.result() else {
            return Ok(ClassHash(StarkFelt::ZERO.into()));
        };
        let class_hash: StarkHash = result.as_str().try_into().unwrap();
        Ok(ClassHash(class_hash))
    }

    pub fn starknet_get_storage_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
        key: &StorageKey,
    ) -> Result<StarkFelt, DatabaseError> {
        let parameters =
            json!({ "block_id": block_number, "contract_address": contract_address, "key": key });
        let args = to_raw_value(&parameters).unwrap();
        let response = self.send_request("starknet_getStorageAt", Some(args.deref()))?;
        let result: String = response.result()?;
        let storage_value: StarkFelt = result.as_str().try_into().unwrap();
        Ok(storage_value)
    }

    fn chain_info(&self) -> ChainInfo {
        // NOTE: these are the same for _all_ networks
        let eth_fee_token_address =
            contract_address!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
        let strk_fee_token_address =
            contract_address!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

        // TODO: Allow different chains.
        let chain_id = ChainId("SN_MAIN".to_string());

        ChainInfo {
            chain_id,
            fee_token_addresses: blockifier::context::FeeTokenAddresses {
                strk_fee_token_address,
                eth_fee_token_address,
            },
        }
    }

    fn block_info(
        &self,
        header: &BlockHeader,
        allow_use_kzg_data: bool,
    ) -> Result<BlockInfo, DatabaseError> {
        Ok(BlockInfo {
            block_number: header.block_number,
            block_timestamp: header.timestamp,
            sequencer_address: header.sequencer.0,
            gas_prices: blockifier::block::GasPrices {
                eth_l1_gas_price: if header.l1_gas_price.price_in_wei.0 == 0 {
                    // Bad API design - the genesis block has 0 gas price, but
                    // blockifier doesn't allow for it. This isn't critical for
                    // consensus, so we just use 1.
                    1.try_into().unwrap()
                } else {
                    NonZeroU128::new(header.l1_gas_price.price_in_wei.0).unwrap()
                },
                strk_l1_gas_price: if header.l1_gas_price.price_in_fri.0 == 0 {
                    // Bad API design - the genesis block has 0 gas price, but
                    // blockifier doesn't allow for it. This isn't critical for
                    // consensus, so we just use 1.
                    1.try_into().unwrap()
                } else {
                    NonZeroU128::new(header.l1_gas_price.price_in_fri.0).unwrap()
                },
                eth_l1_data_gas_price: if header.l1_data_gas_price.price_in_wei.0 == 0 {
                    // Bad API design - pre-v0.13.1 blocks have 0 data gas price, but
                    // blockifier doesn't allow for it. This value is ignored for those
                    // transactions.
                    1.try_into().unwrap()
                } else {
                    NonZeroU128::new(header.l1_data_gas_price.price_in_wei.0).unwrap()
                },
                strk_l1_data_gas_price: if header.l1_data_gas_price.price_in_fri.0 == 0 {
                    // Bad API design - pre-v0.13.1 blocks have 0 data gas price, but
                    // blockifier doesn't allow for it. This value is ignored for those
                    // transactions.
                    1.try_into().unwrap()
                } else {
                    NonZeroU128::new(header.l1_data_gas_price.price_in_fri.0).unwrap()
                },
            },
            use_kzg_da: allow_use_kzg_data && header.l1_da_mode == L1DataAvailabilityMode::Blob,
        })
    }

    fn versioned_constants(&self, header: &BlockHeader) -> &VersionedConstants {
        let starknet_version = header.starknet_version.clone();
        if starknet_version < StarknetVersion("0.13.1.0".to_string()) {
            todo!()
        } else if starknet_version < StarknetVersion("0.13.1.1".to_string()) {
            todo!()
        } else {
            VersionedConstants::latest_constants()
        }
    }
}
impl ReplayStorage for RpcStorage {
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let block_number = self.starknet_block_number()?;
        Ok(BlockNumber::new(block_number))
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

    fn execute_block<S: StateReader>(
        &self,
        work: &ReplayBlock,
    ) -> Result<Vec<(TransactionExecutionInfo, &CachedState<S>)>, RunnerError> {
        let block_number = BlockNumber::new(work.header.block_number.0);

        let mut transactions: Vec<BlockifierTransaction> =
            Vec::with_capacity(work.transactions.len());
        for (transaction, receipt) in work.transactions.iter().zip(work.receipts.iter()) {
            let tx = transaction;
            let tx_hash = receipt.transaction_hash;
            let class_info: Option<ClassInfo> = match tx {
                Transaction::Declare(tx) => match tx {
                    DeclareTransaction::V0(_) => {
                        todo!()
                    }
                    DeclareTransaction::V1(_) => {
                        todo!()
                    }
                    DeclareTransaction::V2(tx) => {
                        let class_hash = tx.class_hash;
                        let replay_class_hash = ReplayClassHash {
                            block_number,
                            class_hash,
                        };
                        let class_definition = self.starknet_get_class(&replay_class_hash)?;
                        let class_info = match class_definition {
                            ContractClass::Sierra(flattened_sierra_cc) => {
                                let mut contract_class =
                                    serde_json::to_value(flattened_sierra_cc.clone()).unwrap();
                                contract_class.as_object_mut().unwrap().remove("abi");
                                let sierra_cc: CairoContractClass =
                                    serde_json::from_value(contract_class).unwrap();
                                let casm_definition = CasmContractClass::from_contract_class(
                                    sierra_cc,
                                    false,
                                    usize::MAX,
                                )
                                .unwrap();
                                let contract_class: ContractClassV1 =
                                    casm_definition.try_into().unwrap();
                                ClassInfo::new(
                                    &BlockifierContractClass::V1(contract_class),
                                    flattened_sierra_cc.sierra_program.len(),
                                    flattened_sierra_cc.abi.len(),
                                )
                            }
                            ContractClass::Legacy(_) => unreachable!(),
                        };
                        Some(class_info.unwrap())
                    }
                    DeclareTransaction::V3(tx) => {
                        let class_hash = tx.class_hash;
                        let replay_class_hash = ReplayClassHash {
                            block_number,
                            class_hash,
                        };
                        let class_definition = self.starknet_get_class(&replay_class_hash)?;
                        let class_info = match class_definition {
                            ContractClass::Sierra(flattened_sierra_cc) => {
                                let mut contract_class =
                                    serde_json::to_value(flattened_sierra_cc.clone()).unwrap();
                                contract_class.as_object_mut().unwrap().remove("abi");
                                let sierra_cc: CairoContractClass =
                                    serde_json::from_value(contract_class).unwrap();
                                let casm_definition = CasmContractClass::from_contract_class(
                                    sierra_cc,
                                    false,
                                    usize::MAX,
                                )
                                .unwrap();
                                let contract_class: ContractClassV1 =
                                    casm_definition.try_into().unwrap();
                                ClassInfo::new(
                                    &BlockifierContractClass::V1(contract_class),
                                    flattened_sierra_cc.sierra_program.len(),
                                    flattened_sierra_cc.abi.len(),
                                )
                            }
                            ContractClass::Legacy(_) => unreachable!(),
                        };
                        Some(class_info.unwrap())
                    }
                },
                _ => None,
            };

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
            )
            .unwrap();

            transactions.push(transaction);
        }

        // I need to call the function transaction.execute from blockifier
        let state_reader =
            ReplayStateReader::new(self, BlockNumber::new(work.header.block_number.0));
        let charge_fee = false;
        let validate = false;
        let allow_use_kzg_data = true;
        let chain_info = self.chain_info();
        let block_info = self.block_info(&work.header, allow_use_kzg_data)?;
        let old_block_number_and_hash = if work.header.block_number.0 >= 10 {
            let block_number_whose_hash_becomes_available =
                BlockNumber::new(work.header.block_number.0 - 10);
            let block_hash = self
                .starknet_get_block_with_tx_hashes(&block_number_whose_hash_becomes_available)
                .unwrap()
                .block_hash;

            Some(BlockNumberHashPair::new(
                block_number_whose_hash_becomes_available.get(),
                block_hash.0,
            ))
        } else {
            None
        };
        let versioned_constants = self.versioned_constants(&work.header);
        let cache_size = 16;
        let mut cached_state = CachedState::new(state_reader, GlobalContractCache::new(cache_size));
        let block_context = pre_process_block(
            &mut cached_state,
            old_block_number_and_hash,
            block_info,
            chain_info,
            versioned_constants.clone(),
        )
        .unwrap();
        for transaction in transactions {
            transaction
                .execute(&mut cached_state, &block_context, charge_fee, validate)
                .unwrap();
        }
        todo!()
    }
}

#[cfg(test)]
mod tests {

    use starknet::core::types::FieldElement;
    use starknet_api::hash::StarkHash;

    use super::*;

    fn build_rpc_storage() -> RpcStorage {
        //let endpoint: Url = Url::parse("https://starknet-mainnet.public.blastapi.io").unwrap();
        let endpoint: Url =
            Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
        RpcStorage::new(endpoint).unwrap()
    }

    #[test]
    fn test_block_number() {
        let rpc_storage = build_rpc_storage();
        let block_number = rpc_storage.starknet_block_number().unwrap();
        // Mainnet block is more than 600k at the moment.
        assert!(block_number > 600000);
    }

    #[test]
    fn test_get_class() {
        let rpc_storage = build_rpc_storage();
        let class_hash: StarkHash =
            "0x029927c8af6bccf3f6fda035981e765a7bdbf18a2dc0d630494f8758aa908e2b"
                .try_into()
                .unwrap();
        let replay_class_hash = ReplayClassHash {
            block_number: BlockNumber::new(632917),
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
        let block_number = BlockNumber::new(632917);
        let block_header = rpc_storage
            .starknet_get_block_with_tx_hashes(&block_number)
            .unwrap();
        assert_eq!(block_header.timestamp.0, 1713168820);
    }

    #[test]
    fn test_get_block_with_receipts() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632917);
        let transactions = rpc_storage
            .starknet_get_block_with_receipts(&block_number)
            .unwrap();
    }

    #[test]
    fn test_get_nonce() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632917);
        let contract_address: ContractAddress =
            contract_address!("0x0710ce97d91835e049e5e76fbcb594065405744cf057b5b5f553282108983c53");
        let nonce = rpc_storage
            .starknet_get_nonce(&block_number, &contract_address)
            .unwrap();

        let nonce_expected: StarkHash = "0x4".try_into().unwrap();
        let nonce_expected = Nonce(nonce_expected);

        assert_eq!(nonce, nonce_expected)
    }

    #[test]
    fn test_get_class_hash_at() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632916);
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
    fn test_get_storage_at() {
        let rpc_storage = build_rpc_storage();
        let block_number = BlockNumber::new(632917);
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
}
