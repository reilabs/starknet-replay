use std::collections::HashMap;

use primitive_types::H160;
use starknet_api::block::BlockHash;
use starknet_api::core::{ContractAddress, EthAddress};
use starknet_api::transaction::{
    Builtin,
    DeclareTransactionOutput,
    DeployAccountTransactionOutput,
    DeployTransactionOutput,
    Event,
    EventContent,
    EventData,
    EventKey,
    ExecutionResources,
    Fee,
    GasVector,
    InvokeTransactionOutput,
    L1HandlerTransactionOutput,
    L2ToL1Payload,
    MessageToL1,
    TransactionExecutionStatus,
    TransactionHash,
    TransactionOutput,
    TransactionReceipt as StarknetApiReceipt,
};
use starknet_core::types::{
    ComputationResources,
    ExecutionResult,
    Felt,
    MsgToL1,
    TransactionReceipt as StarknetCoreReceipt,
};

use crate::error::DatabaseError;

fn generate_builtin_counter(computation_resources: &ComputationResources) -> HashMap<Builtin, u64> {
    let mut builtin_instance_counter = HashMap::default();
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::RangeCheck,
        computation_resources
            .range_check_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::Pedersen,
        computation_resources
            .pedersen_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::Poseidon,
        computation_resources
            .poseidon_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::EcOp,
        computation_resources
            .ec_op_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::Ecdsa,
        computation_resources
            .ecdsa_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::Bitwise,
        computation_resources
            .bitwise_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::Keccak,
        computation_resources
            .keccak_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::transaction::Builtin::SegmentArena,
        computation_resources
            .segment_arena_builtin
            .unwrap_or_default(),
    );
    builtin_instance_counter
}

fn generate_events(
    events: Vec<starknet_core::types::Event>,
) -> Vec<starknet_api::transaction::Event> {
    events
        .into_iter()
        .map(|e| Event {
            from_address: ContractAddress(e.from_address.try_into().unwrap()),
            content: EventContent {
                keys: e.keys.into_iter().map(|k| EventKey(k)).collect(),
                data: EventData(e.data),
            },
        })
        .collect()
}

fn generate_messages(messages_sent: Vec<MsgToL1>) -> Vec<MessageToL1> {
    messages_sent
        .into_iter()
        .map(|m| MessageToL1 {
            from_address: ContractAddress(m.from_address.try_into().unwrap()),
            to_address: {
                let bytes = m.to_address.to_bytes_be();
                let (_, h160_bytes) = bytes.split_at(12);
                EthAddress(H160::from_slice(h160_bytes))
            },
            payload: L2ToL1Payload(m.payload),
        })
        .collect()
}

fn generate_execution_resources(
    execution_resources: starknet_core::types::ExecutionResources,
) -> ExecutionResources {
    ExecutionResources {
        steps: execution_resources.computation_resources.steps,
        builtin_instance_counter: generate_builtin_counter(
            &execution_resources.computation_resources,
        ),
        memory_holes: execution_resources
            .computation_resources
            .memory_holes
            .unwrap_or_default(),
        da_gas_consumed: GasVector {
            l1_gas: 0,      // Where do I get this data?
            l1_data_gas: 0, // Where do I get this data?
        },
        gas_consumed: GasVector {
            l1_gas: execution_resources.data_resources.data_availability.l1_gas,
            l1_data_gas: execution_resources
                .data_resources
                .data_availability
                .l1_data_gas,
        },
    }
}

fn generate_execution_status(execution_result: ExecutionResult) -> TransactionExecutionStatus {
    match execution_result {
        starknet_core::types::ExecutionResult::Succeeded => {
            starknet_api::transaction::TransactionExecutionStatus::Succeeded
        }
        starknet_core::types::ExecutionResult::Reverted { reason } => {
            starknet_api::transaction::TransactionExecutionStatus::Reverted(
                starknet_api::transaction::RevertedTransactionExecutionStatus {
                    revert_reason: reason,
                },
            )
        }
    }
}

pub fn convert_receipt(
    block_hash: &Felt,
    block_number: &u64,
    receipt: StarknetCoreReceipt,
) -> Result<StarknetApiReceipt, DatabaseError> {
    let block_hash = BlockHash(Felt::from_bytes_be(&block_hash.to_bytes_be().into()));
    let block_number = starknet_api::block::BlockNumber(block_number.clone());
    match receipt {
        StarknetCoreReceipt::Invoke(receipt) => {
            let tx_output = InvokeTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse().unwrap()),
                messages_sent: generate_messages(receipt.messages_sent),
                events: generate_events(receipt.events),
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash: block_hash.clone(),
                block_number: block_number.clone(),
                output: TransactionOutput::Invoke(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::L1Handler(receipt) => {
            let tx_output = L1HandlerTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse().unwrap()),
                messages_sent: generate_messages(receipt.messages_sent),
                events: generate_events(receipt.events),
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash: block_hash.clone(),
                block_number: block_number.clone(),
                output: TransactionOutput::L1Handler(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::Declare(receipt) => {
            let tx_output = DeclareTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse().unwrap()),
                messages_sent: generate_messages(receipt.messages_sent),
                events: generate_events(receipt.events),
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash: block_hash.clone(),
                block_number: block_number.clone(),
                output: TransactionOutput::Declare(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::Deploy(receipt) => {
            let tx_output = DeployTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse().unwrap()),
                messages_sent: generate_messages(receipt.messages_sent),
                events: generate_events(receipt.events),
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(receipt.execution_resources),
                contract_address: ContractAddress(receipt.contract_address.try_into().unwrap()),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash: block_hash.clone(),
                block_number: block_number.clone(),
                output: TransactionOutput::Deploy(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::DeployAccount(receipt) => {
            let tx_output = DeployAccountTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse().unwrap()),
                messages_sent: generate_messages(receipt.messages_sent),
                events: generate_events(receipt.events),
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(receipt.execution_resources),
                contract_address: ContractAddress(receipt.contract_address.try_into().unwrap()),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash: block_hash.clone(),
                block_number: block_number.clone(),
                output: TransactionOutput::DeployAccount(tx_output),
            };
            Ok(receipt)
        }
    }
}
