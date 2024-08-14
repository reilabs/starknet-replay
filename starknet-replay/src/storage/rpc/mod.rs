//! This module uses the Starknet RPC protocol to query the data required to
//! replay transactions from the Starknet blockchain.

#![allow(clippy::module_name_repetitions)] // Added because of `ClassInfo`

use std::num::NonZeroU128;

use blockifier::blockifier::block::{pre_process_block, BlockInfo, BlockNumberHashPair, GasPrices};
use blockifier::bouncer::BouncerConfig;
use blockifier::context::{BlockContext, ChainInfo};
use blockifier::state::cached_state::CachedState;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use blockifier::versioned_constants::VersionedConstants;
use once_cell::sync::Lazy;
use starknet_api::block::{
    BlockHash,
    BlockHeader,
    BlockTimestamp,
    GasPrice,
    GasPricePerToken,
    StarknetVersion,
};
use starknet_api::core::{
    ChainId,
    ClassHash,
    ContractAddress,
    GlobalRoot,
    Nonce,
    PatriciaKey,
    SequencerContractAddress,
};
use starknet_api::data_availability::L1DataAvailabilityMode;
use starknet_api::hash::StarkHash;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{Transaction, TransactionReceipt};
use starknet_api::{contract_address, felt, patricia_key};
use starknet_core::types::{
    BlockId,
    ContractClass,
    Felt,
    MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes,
};
use starknet_providers::jsonrpc::HttpTransport;
use starknet_providers::{JsonRpcClient, Provider};
use url::Url;

use crate::block_number::BlockNumber;
use crate::contract_address::to_field_element;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::{ReplayClassHash, TransactionOutput, VisitedPcs};
use crate::runner::replay_state_reader::ReplayStateReader;
use crate::storage::rpc::receipt::convert_receipt;
use crate::storage::rpc::transaction::convert_transaction;
use crate::storage::Storage as ReplayStorage;

pub mod class_info;
pub mod contract_class;
pub mod receipt;
pub mod transaction;

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
    endpoint: Url,

    /// The client field sends RPC calls.
    client: JsonRpcClient<HttpTransport>,
}
impl RpcStorage {
    /// Constructs a new `RpcStorage`.
    ///
    /// # Arguments
    ///
    /// - `endpoint`: The Url of the Starknet RPC node.
    #[must_use]
    pub fn new(endpoint: Url) -> Self {
        let client = JsonRpcClient::new(HttpTransport::new(endpoint.clone()));
        RpcStorage { endpoint, client }
    }

    /// This function queries the number of the most recent Starknet block.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let block_number: u64 = self.client.block_number().await?;
        Ok(BlockNumber::new(block_number))
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_class(
        &self,
        class_hash_at_block: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        let block_id: BlockId = class_hash_at_block.block_number.into();
        let class_hash: Felt = class_hash_at_block.class_hash.0;
        let contract_class: ContractClass = self.client.get_class(block_id, class_hash).await?;
        Ok(contract_class)
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_block_with_tx_hashes(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockHeader, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let block_header: MaybePendingBlockWithTxHashes =
            self.client.get_block_with_tx_hashes(block_id).await?;
        match block_header {
            MaybePendingBlockWithTxHashes::Block(block_header) => {
                let sequencer: StarkHash =
                    Felt::from_bytes_be(&block_header.sequencer_address.to_bytes_be());
                let price_in_fri: u128 =
                    block_header.l1_gas_price.price_in_fri.to_string().parse()?;
                let price_in_wei: u128 =
                    block_header.l1_gas_price.price_in_wei.to_string().parse()?;

                let data_price_in_fri: u128 = block_header
                    .l1_data_gas_price
                    .price_in_fri
                    .to_string()
                    .parse()?;
                let data_price_in_wei: u128 = block_header
                    .l1_data_gas_price
                    .price_in_wei
                    .to_string()
                    .parse()?;

                let block_header: BlockHeader = BlockHeader {
                    block_hash: BlockHash(Felt::from_bytes_be(
                        &block_header.block_hash.to_bytes_be(),
                    )),
                    parent_hash: BlockHash(Felt::from_bytes_be(
                        &block_header.parent_hash.to_bytes_be(),
                    )),
                    block_number: starknet_api::block::BlockNumber(block_header.block_number),
                    l1_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(price_in_fri),
                        price_in_wei: GasPrice(price_in_wei),
                    },
                    l1_data_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(data_price_in_fri),
                        price_in_wei: GasPrice(data_price_in_wei),
                    },
                    state_root: GlobalRoot(Felt::from_bytes_be(
                        &block_header.new_root.to_bytes_be(),
                    )),
                    sequencer: SequencerContractAddress(sequencer.try_into()?),
                    timestamp: BlockTimestamp(block_header.timestamp),
                    l1_da_mode: match block_header.l1_da_mode {
                        starknet_core::types::L1DataAvailabilityMode::Blob => {
                            L1DataAvailabilityMode::Blob
                        }
                        starknet_core::types::L1DataAvailabilityMode::Calldata => {
                            L1DataAvailabilityMode::Calldata
                        }
                    },
                    state_diff_commitment: None,
                    transaction_commitment: None,
                    event_commitment: None,
                    n_transactions: block_header.transactions.len(),
                    n_events: 0,
                    starknet_version: StarknetVersion(block_header.starknet_version),
                    state_diff_length: None,
                    receipt_commitment: None,
                };
                Ok(block_header)
            }
            MaybePendingBlockWithTxHashes::PendingBlock(_) => unreachable!(),
        }
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_block_with_receipts(
        &self,
        block_number: &BlockNumber,
    ) -> Result<(Vec<Transaction>, Vec<TransactionReceipt>), DatabaseError> {
        let block_id: BlockId = block_number.into();
        let txs_with_receipts: MaybePendingBlockWithReceipts =
            self.client.get_block_with_receipts(block_id).await?;
        match txs_with_receipts {
            MaybePendingBlockWithReceipts::Block(block) => {
                let mut transactions: Vec<Transaction> =
                    Vec::with_capacity(block.transactions.len());
                let mut receipts: Vec<TransactionReceipt> =
                    Vec::with_capacity(block.transactions.len());

                for tx in block.transactions {
                    let transaction = convert_transaction(tx.transaction)?;
                    let receipt =
                        convert_receipt(&block.block_hash, &block.block_number, tx.receipt)?;
                    transactions.push(transaction);
                    receipts.push(receipt);
                }
                Ok((transactions, receipts))
            }
            MaybePendingBlockWithReceipts::PendingBlock(_) => unreachable!(),
        }
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_nonce(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<Nonce, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let nonce: Felt = self.client.get_nonce(block_id, contract_address).await?;
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_class_hash_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<ClassHash, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let class_hash: Felt = self
            .client
            .get_class_hash_at(block_id, contract_address)
            .await?;
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
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_storage_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
        key: &StorageKey,
    ) -> Result<Felt, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let key: Felt = to_field_element(key);
        let storage_value: Felt = self
            .client
            .get_storage_at(contract_address, key, block_id)
            .await?;
        Ok(storage_value)
    }

    /// This function queries the chain id of the RPC endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or decoding hex values of the chain
    /// id fails.
    #[allow(clippy::missing_panics_doc)] // To avoid false positives caused by OK() statement.
    #[tokio::main]
    pub async fn starknet_get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        let chain_id: Felt = self.client.chain_id().await?;
        let chain_id = chain_id.to_hex_string();
        let chain_id: Vec<&str> = chain_id.split("0x").collect();
        let decoded_result = hex::decode(chain_id.last().ok_or(DatabaseError::InvalidHex())?)?;
        let chain_id = std::str::from_utf8(&decoded_result)?;
        let chain_id = ChainId::from(chain_id.to_string());
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
        let mut state = CachedState::new(state_reader);
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
        for transaction in transactions {
            let mut tx_state = CachedState::<_>::create_transactional(&mut state);
            // No fee is being calculated.
            let tx_info = transaction.execute(&mut tx_state, &block_context, charge_fee, validate);
            tx_state.to_state_diff()?;
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
                            (replay_class_hash, pcs.into_iter().collect())
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

    use starknet_api::felt;
    use starknet_api::hash::StarkHash;

    use super::*;

    fn build_rpc_storage() -> RpcStorage {
        let endpoint: Url =
            Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
        RpcStorage::new(endpoint)
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
            Felt::from_hex("0x029927c8af6bccf3f6fda035981e765a7bdbf18a2dc0d630494f8758aa908e2b")
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
            .starknet_get_storage_at(&block_number, &contract_address, &storage_key)
            .unwrap();

        let storage_value_expected: Felt = Felt::from_hex("0x397de273516b4").unwrap();
        assert_eq!(storage_value, storage_value_expected);
    }

    #[test]
    fn test_get_chain_id() {
        let rpc_storage = build_rpc_storage();
        let main_chain = ChainId::from("SN_MAIN".to_string());
        let chain_id = rpc_storage.starknet_get_chain_id().unwrap();
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
