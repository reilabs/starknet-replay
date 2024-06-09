use std::path::PathBuf;

use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use papyrus_execution::objects::{PendingData, TransactionSimulationOutput};
use papyrus_execution::{simulate_transactions, ExecutableTransactionInput, ExecutionConfig};
use papyrus_node::config::NodeConfig;
use papyrus_storage::body::events::ThinTransactionOutput;
use papyrus_storage::body::BodyStorageReader;
use papyrus_storage::class::ClassStorageReader;
use papyrus_storage::compiled_class::CasmStorageReader;
use papyrus_storage::header::HeaderStorageReader;
use papyrus_storage::state::StateStorageReader;
use papyrus_storage::{open_storage, StorageReader};
use starknet_api::block::BlockHeader;
use starknet_api::core::{ChainId, ContractAddress, PatriciaKey};
use starknet_api::deprecated_contract_class::ContractClass as DeprecatedContractClass;
use starknet_api::hash::StarkHash;
use starknet_api::state::{ContractClass as StarknetContractClass, StateNumber};
use starknet_api::transaction::{
    DeclareTransactionOutput,
    DeployAccountTransactionOutput,
    DeployTransactionOutput,
    Fee,
    InvokeTransactionOutput,
    L1HandlerTransactionOutput,
    Transaction as StarknetTransaction,
    TransactionHash,
    TransactionOutput,
};
use starknet_api::{contract_address, patricia_key};

use crate::common::contract_class::ContractClass;
use crate::common::storage::Storage as ReplayStorage;
use crate::common::BlockNumber;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::runner::VisitedPcs;

pub mod block_number;
pub mod contract_class;

#[derive(Clone)]
pub struct PapyrusStorage {
    pub storage_reader: StorageReader,
}
impl PapyrusStorage {
    pub fn new(database_path: PathBuf) -> Self {
        let mut config = NodeConfig::default();
        config.storage.db_config.path_prefix = database_path;
        // Igoring `storage_writer` because this application requires only reading.
        let (storage_reader, _storage_writer) = open_storage(config.storage.clone()).unwrap();
        Self { storage_reader }
    }

    fn get_test_execution_config() -> ExecutionConfig {
        ExecutionConfig {
            strk_fee_contract_address: contract_address!("0x1001"),
            eth_fee_contract_address: contract_address!("0x1001"),
            initial_gas_cost: 10_u64.pow(10),
        }
    }

    fn execute_simulate_transactions(
        storage_reader: StorageReader,
        maybe_pending_data: Option<PendingData>,
        txs: Vec<ExecutableTransactionInput>,
        tx_hashes: Option<Vec<TransactionHash>>,
        charge_fee: bool,
        validate: bool,
    ) -> Vec<TransactionSimulationOutput> {
        let chain_id = ChainId(String::from("SN_MAIN"));
        //let chain_id = ChainId(CHAIN_ID.to_string());

        simulate_transactions(
            txs,
            tx_hashes,
            &chain_id,
            storage_reader,
            maybe_pending_data,
            StateNumber::unchecked_right_after_block(BlockNumber::new(632916).into()),
            BlockNumber::new(632917).into(),
            &Self::get_test_execution_config(),
            charge_fee,
            validate,
            // TODO: Consider testing without overriding DA (It's already tested in the RPC)
            true,
        )
        .unwrap()
    }

    fn get_class_definition_at(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<StarknetContractClass, DatabaseError> {
        let state_number_after_block =
            StateNumber::unchecked_right_after_block(replay_class_hash.block_number.into());
        let contract_class = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_state_reader()
            .unwrap()
            .get_class_definition_at(
                state_number_after_block,
                &replay_class_hash.class_hash.into(),
            )
            .unwrap()
            .unwrap();
        Ok(contract_class)
    }

    fn get_deprecated_class_definition_at(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<DeprecatedContractClass, DatabaseError> {
        let state_number_after_block =
            StateNumber::unchecked_right_after_block(replay_class_hash.block_number.into());
        let contract_class = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_state_reader()
            .unwrap()
            .get_deprecated_class_definition_at(
                state_number_after_block,
                &replay_class_hash.class_hash.into(),
            )
            .unwrap()
            .unwrap();
        Ok(contract_class)
    }

    fn get_casm(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<CasmContractClass, DatabaseError> {
        let casm = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_casm(&replay_class_hash.class_hash.into())
            .unwrap()
            .unwrap();
        Ok(casm)
    }
}
impl ReplayStorage for PapyrusStorage {
    fn get_latest_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let next_block_number = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_header_marker()
            .unwrap();
        let block_number: BlockNumber = BlockNumber::new(next_block_number.0 - 1);
        Ok(block_number)
    }

    fn get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        todo!()
    }

    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        // TODO: Should I check the block is less than what returned by
        // `get_class_marker`?
        let class_hash = replay_class_hash.class_hash;
        let contract_class = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_class(&class_hash)
            .unwrap()
            .unwrap();

        Ok(contract_class.into())
    }

    fn get_block_header(&self, block_number: BlockNumber) -> Result<BlockHeader, DatabaseError> {
        let block_header = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_block_header(block_number.into())
            .unwrap()
            .unwrap();
        Ok(block_header)
    }

    fn get_transactions_and_receipts_for_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<(Vec<StarknetTransaction>, Vec<TransactionOutput>), DatabaseError> {
        let transactions = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_block_transactions(block_number.into())
            .unwrap()
            .unwrap();
        let transaction_outputs = self
            .storage_reader
            .begin_ro_txn()
            .unwrap()
            .get_block_transaction_outputs(block_number.into())
            .unwrap()
            .unwrap();
        let transaction_outputs = transaction_outputs
            .iter()
            .map(|t| match t.clone() {
                ThinTransactionOutput::Declare(tx_output) => {
                    TransactionOutput::Declare(DeclareTransactionOutput {
                        actual_fee: tx_output.actual_fee,
                        messages_sent: tx_output.messages_sent,
                        events: Vec::new(),
                        execution_status: tx_output.execution_status,
                        execution_resources: tx_output.execution_resources,
                    })
                }
                ThinTransactionOutput::Deploy(tx_output) => {
                    TransactionOutput::Deploy(DeployTransactionOutput {
                        actual_fee: tx_output.actual_fee,
                        messages_sent: tx_output.messages_sent,
                        events: Vec::new(),
                        contract_address: tx_output.contract_address,
                        execution_status: tx_output.execution_status,
                        execution_resources: tx_output.execution_resources,
                    })
                }
                ThinTransactionOutput::DeployAccount(tx_output) => {
                    TransactionOutput::DeployAccount(DeployAccountTransactionOutput {
                        actual_fee: tx_output.actual_fee,
                        messages_sent: tx_output.messages_sent,
                        events: Vec::new(),
                        contract_address: tx_output.contract_address,
                        execution_status: tx_output.execution_status,
                        execution_resources: tx_output.execution_resources,
                    })
                }
                ThinTransactionOutput::Invoke(tx_output) => {
                    TransactionOutput::Invoke(InvokeTransactionOutput {
                        actual_fee: tx_output.actual_fee,
                        messages_sent: tx_output.messages_sent,
                        events: Vec::new(),
                        execution_status: tx_output.execution_status,
                        execution_resources: tx_output.execution_resources,
                    })
                }
                ThinTransactionOutput::L1Handler(tx_output) => {
                    TransactionOutput::L1Handler(L1HandlerTransactionOutput {
                        actual_fee: tx_output.actual_fee,
                        messages_sent: tx_output.messages_sent,
                        events: Vec::new(),
                        execution_status: tx_output.execution_status,
                        execution_resources: tx_output.execution_resources,
                    })
                }
            })
            .collect();
        Ok((transactions, transaction_outputs))
    }

    fn execute_block(&self, work: &ReplayBlock) -> Result<VisitedPcs, RunnerError> {
        let maybe_pending_data = None;
        let only_query = true;
        let fee = Fee(10);
        let txs: Vec<ExecutableTransactionInput> = work
            .transactions
            .iter()
            .map(|t| match t.clone() {
                StarknetTransaction::Declare(tx) => {
                    //ExecutableTransactionInput::Declare(tx, only_query)
                    match tx {
                        starknet_api::transaction::DeclareTransaction::V0(tx) => {
                            let replay_class_hash = ReplayClassHash {
                                block_number: work.header.block_number.into(),
                                class_hash: tx.class_hash,
                            };
                            let class_definition =
                                self.get_class_definition_at(&replay_class_hash).unwrap();
                            let deprecated_class_definition = self
                                .get_deprecated_class_definition_at(&replay_class_hash)
                                .unwrap();
                            ExecutableTransactionInput::DeclareV0(
                                tx,
                                deprecated_class_definition,
                                class_definition.abi.len(),
                                only_query,
                            )
                        }
                        starknet_api::transaction::DeclareTransaction::V1(tx) => {
                            let replay_class_hash = ReplayClassHash {
                                block_number: work.header.block_number.into(),
                                class_hash: tx.class_hash,
                            };
                            let class_definition =
                                self.get_class_definition_at(&replay_class_hash).unwrap();
                            let deprecated_class_definition = self
                                .get_deprecated_class_definition_at(&replay_class_hash)
                                .unwrap();
                            ExecutableTransactionInput::DeclareV1(
                                tx,
                                deprecated_class_definition,
                                class_definition.abi.len(),
                                only_query,
                            )
                        }
                        starknet_api::transaction::DeclareTransaction::V2(tx) => {
                            let replay_class_hash = ReplayClassHash {
                                block_number: work.header.block_number.into(),
                                class_hash: tx.class_hash,
                            };
                            let class_definition =
                                self.get_class_definition_at(&replay_class_hash).unwrap();
                            let casm = self.get_casm(&replay_class_hash).unwrap();
                            ExecutableTransactionInput::DeclareV2(
                                tx,
                                casm,
                                class_definition.sierra_program.len(),
                                class_definition.abi.len(),
                                only_query,
                            )
                        }
                        starknet_api::transaction::DeclareTransaction::V3(tx) => {
                            let replay_class_hash = ReplayClassHash {
                                block_number: work.header.block_number.into(),
                                class_hash: tx.class_hash,
                            };
                            let class_definition =
                                self.get_class_definition_at(&replay_class_hash).unwrap();
                            let casm = self.get_casm(&replay_class_hash).unwrap();
                            ExecutableTransactionInput::DeclareV3(
                                tx,
                                casm,
                                class_definition.sierra_program.len(),
                                class_definition.abi.len(),
                                only_query,
                            )
                        }
                    }
                }
                StarknetTransaction::Deploy(_tx) => {
                    todo!()
                    //ExecutableTransactionInput::Deploy(tx, only_query)
                }
                StarknetTransaction::DeployAccount(tx) => {
                    ExecutableTransactionInput::DeployAccount(tx, only_query)
                }
                StarknetTransaction::Invoke(tx) => {
                    println!("{:#?}", tx);
                    ExecutableTransactionInput::Invoke(tx, only_query)
                }
                StarknetTransaction::L1Handler(tx) => {
                    ExecutableTransactionInput::L1Handler(tx, fee, only_query)
                }
            })
            .collect();
        //let tx_hashes: Vec<TransactionHash> = work.receipts;
        let tx_hashes = None;
        let charge_fee = false;
        let validate = false;
        Self::execute_simulate_transactions(
            self.storage_reader.clone(),
            maybe_pending_data,
            txs,
            tx_hashes,
            charge_fee,
            validate,
        );
        todo!()
    }
}
