//! This module uses the Starknet RPC protocol to query the data from the
//! Starknet RPC server.

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
    SequencerContractAddress,
};
use starknet_api::data_availability::L1DataAvailabilityMode;
use starknet_api::hash::StarkHash;
use starknet_api::state::StorageKey;
use starknet_core::types::{
    BlockId,
    ContractClass,
    Felt,
    MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes,
    StarknetError,
};
use starknet_providers::jsonrpc::HttpTransport;
use starknet_providers::{JsonRpcClient, Provider, ProviderError};
use tokio::sync::OnceCell;
use tracing::trace;
use url::Url;

use crate::block_number::BlockNumber;
use crate::contract_address::to_field_element;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::rpc::receipt::convert_receipt;
use crate::storage::rpc::transaction::convert_transaction;
use crate::storage::BlockWithReceipts;

/// This structure partially implements a Starknet RPC client.
///
/// The RPC calls included are those needed to replay transactions.
/// Clone is not derived because it's not supported by Client.
pub struct RpcClient {
    /// The endpoint of the Starknet RPC Node.
    endpoint: Url,

    /// The chain id variable initialised with the first call to
    /// [`RpcClient::starknet_get_chain_id`]. This is doable because it's not
    /// possible to replay blocks from different chains.
    chain_id: OnceCell<ChainId>,

    /// The most recent block number available from the rpc server. It is
    /// initialised in [`RpcClient::starknet_block_number`]. The assumption
    /// is that the latest blocks generated after the replay is started are
    /// ignored.
    block_number: OnceCell<BlockNumber>,
}
impl RpcClient {
    /// Constructs a new `RpcStorage`.
    ///
    /// # Arguments
    ///
    /// - `endpoint`: The Url of the Starknet RPC node.
    #[must_use]
    pub fn new(endpoint: Url) -> Self {
        RpcClient {
            endpoint,
            chain_id: OnceCell::new(),
            block_number: OnceCell::new(),
        }
    }

    /// This function generates a new client to perform an RPC request to the
    /// endpoint.
    ///
    /// The client can't be shared across threads.
    fn get_new_client(&self) -> JsonRpcClient<HttpTransport> {
        JsonRpcClient::new(HttpTransport::new(self.endpoint.clone()))
    }

    /// This function queries the number of the most recent Starknet block.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails.
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_block_number(&self) -> Result<BlockNumber, ProviderError> {
        let block_number: Result<&BlockNumber, ProviderError> = self
            .block_number
            .get_or_try_init(|| async {
                let block_number: u64 = self.get_new_client().block_number().await?;
                Ok(BlockNumber::new(block_number))
            })
            .await;
        Ok(block_number?).cloned()
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_class(
        &self,
        class_hash_at_block: &ReplayClassHash,
    ) -> Result<ContractClass, ProviderError> {
        let block_id: BlockId = class_hash_at_block.block_number.into();
        let class_hash: Felt = class_hash_at_block.class_hash.0;
        let contract_class: ContractClass = self
            .get_new_client()
            .get_class(block_id, class_hash)
            .await?;
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_block_with_tx_hashes(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockHeader, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let block_header: MaybePendingBlockWithTxHashes = self
            .get_new_client()
            .get_block_with_tx_hashes(block_id)
            .await?;
        match block_header {
            MaybePendingBlockWithTxHashes::Block(block) => {
                let sequencer: StarkHash =
                    Felt::from_bytes_be(&block.sequencer_address.to_bytes_be());
                let price_in_fri: u128 = block.l1_gas_price.price_in_fri.to_string().parse()?;
                let price_in_wei: u128 = block.l1_gas_price.price_in_wei.to_string().parse()?;

                let data_price_in_fri: u128 =
                    block.l1_data_gas_price.price_in_fri.to_string().parse()?;
                let data_price_in_wei: u128 =
                    block.l1_data_gas_price.price_in_wei.to_string().parse()?;

                let block_header = BlockHeader {
                    block_hash: BlockHash(Felt::from_bytes_be(&block.block_hash.to_bytes_be())),
                    parent_hash: BlockHash(Felt::from_bytes_be(&block.parent_hash.to_bytes_be())),
                    block_number: starknet_api::block::BlockNumber(block.block_number),
                    l1_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(price_in_fri),
                        price_in_wei: GasPrice(price_in_wei),
                    },
                    l1_data_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(data_price_in_fri),
                        price_in_wei: GasPrice(data_price_in_wei),
                    },
                    state_root: GlobalRoot(Felt::from_bytes_be(&block.new_root.to_bytes_be())),
                    sequencer: SequencerContractAddress(sequencer.try_into()?),
                    timestamp: BlockTimestamp(block.timestamp),
                    l1_da_mode: match block.l1_da_mode {
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
                    n_transactions: block.transactions.len(),
                    n_events: 0,
                    starknet_version: StarknetVersion(block.starknet_version),
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_block_with_receipts(
        &self,
        block_number: &BlockNumber,
    ) -> Result<BlockWithReceipts, DatabaseError> {
        let block_id: BlockId = block_number.into();
        let txs_with_receipts: MaybePendingBlockWithReceipts = self
            .get_new_client()
            .get_block_with_receipts(block_id)
            .await?;
        match txs_with_receipts {
            MaybePendingBlockWithReceipts::Block(block) => {
                let sequencer: StarkHash =
                    Felt::from_bytes_be(&block.sequencer_address.to_bytes_be());
                let price_in_fri: u128 = block.l1_gas_price.price_in_fri.to_string().parse()?;
                let price_in_wei: u128 = block.l1_gas_price.price_in_wei.to_string().parse()?;

                let data_price_in_fri: u128 =
                    block.l1_data_gas_price.price_in_fri.to_string().parse()?;
                let data_price_in_wei: u128 =
                    block.l1_data_gas_price.price_in_wei.to_string().parse()?;

                let block_header = BlockHeader {
                    block_hash: BlockHash(Felt::from_bytes_be(&block.block_hash.to_bytes_be())),
                    parent_hash: BlockHash(Felt::from_bytes_be(&block.parent_hash.to_bytes_be())),
                    block_number: starknet_api::block::BlockNumber(block.block_number),
                    l1_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(price_in_fri),
                        price_in_wei: GasPrice(price_in_wei),
                    },
                    l1_data_gas_price: GasPricePerToken {
                        price_in_fri: GasPrice(data_price_in_fri),
                        price_in_wei: GasPrice(data_price_in_wei),
                    },
                    state_root: GlobalRoot(Felt::from_bytes_be(&block.new_root.to_bytes_be())),
                    sequencer: SequencerContractAddress(sequencer.try_into()?),
                    timestamp: BlockTimestamp(block.timestamp),
                    l1_da_mode: match block.l1_da_mode {
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
                    n_transactions: block.transactions.len(),
                    n_events: 0,
                    starknet_version: StarknetVersion(block.starknet_version),
                    state_diff_length: None,
                    receipt_commitment: None,
                };

                let mut transactions = Vec::with_capacity(block.transactions.len());
                let mut receipts = Vec::with_capacity(block.transactions.len());

                for tx in block.transactions {
                    let transaction = convert_transaction(tx.transaction)?;
                    let receipt =
                        convert_receipt(&block.block_hash, &block.block_number, tx.receipt)?;
                    transactions.push(transaction);
                    receipts.push(receipt);
                }
                Ok((block_header, transactions, receipts))
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_nonce(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<Nonce, ProviderError> {
        trace!(
            "starknet_get_nonce {:?} {:?}",
            block_number,
            contract_address
        );
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let req = self
            .get_new_client()
            .get_nonce(block_id, contract_address)
            .await;
        match req {
            Ok(nonce) => Ok(Nonce(nonce)),
            Err(err) => match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    Ok(Nonce(Felt::ZERO))
                }
                _ => Err(err),
            },
        }
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_class_hash_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
    ) -> Result<ClassHash, ProviderError> {
        trace!(
            "starknet_get_class_hash_at {:?} {:?}",
            block_number,
            contract_address
        );
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let req = self
            .get_new_client()
            .get_class_hash_at(block_id, contract_address)
            .await;
        match req {
            Ok(class_hash) => Ok(ClassHash(class_hash)),
            Err(err) => match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    Ok(ClassHash(Felt::ZERO))
                }
                _ => Err(err),
            },
        }
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
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_storage_at(
        &self,
        block_number: &BlockNumber,
        contract_address: &ContractAddress,
        key: &StorageKey,
    ) -> Result<Felt, ProviderError> {
        trace!(
            "starknet_get_storage_at {:?} {:?} {:?}",
            block_number,
            contract_address,
            key
        );
        let block_id: BlockId = block_number.into();
        let contract_address: Felt = to_field_element(contract_address);
        let key: Felt = to_field_element(key);
        let req = self
            .get_new_client()
            .get_storage_at(contract_address, key, block_id)
            .await;
        match req {
            Ok(storage_value) => Ok(storage_value),
            Err(err) => match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => Ok(Felt::ZERO),
                _ => Err(err),
            },
        }
    }

    /// This function queries the chain id of the RPC endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the request fails or decoding hex values of the chain
    /// id fails.
    #[allow(clippy::missing_panics_doc)] // Needed because `tokio::main` calls `unwrap()`
    #[tokio::main]
    pub async fn starknet_get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        let chain_id: Result<&ChainId, DatabaseError> = self
            .chain_id
            .get_or_try_init(|| async {
                let chain_id: Felt = self.get_new_client().chain_id().await?;
                let chain_id = chain_id.to_hex_string();
                let chain_id: Vec<&str> = chain_id.split("0x").collect();
                let decoded_result =
                    hex::decode(chain_id.last().ok_or(DatabaseError::InvalidHex())?)?;
                let chain_id = std::str::from_utf8(&decoded_result)?;
                let chain_id = ChainId::from(chain_id.to_string());
                Ok(chain_id)
            })
            .await;
        Ok(chain_id?).cloned()
    }
}
