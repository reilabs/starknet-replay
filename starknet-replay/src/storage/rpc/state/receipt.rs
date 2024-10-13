//! This module contains the functions to generate the transaction receipt from
//! the RPC response.

use std::collections::HashMap;

use primitive_types::H160;
use starknet_api::block::BlockHash;
use starknet_api::core::{ContractAddress, EthAddress};
use starknet_api::execution_resources::{Builtin, ExecutionResources, GasVector};
use starknet_api::transaction::{
    DeclareTransactionOutput,
    DeployAccountTransactionOutput,
    DeployTransactionOutput,
    Event,
    EventContent,
    EventData,
    EventKey,
    Fee,
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

use crate::error::RpcClientError;

/// This function generates a hashmap of builtins usage in a transaction.
///
/// It is needed to generate the object
/// [`starknet_api::execution_resources::ExecutionResources`].
///
/// # Arguments
///
/// - `computation_resources`: The object returned from the RPC call.
fn generate_builtin_counter(computation_resources: &ComputationResources) -> HashMap<Builtin, u64> {
    let mut builtin_instance_counter = HashMap::default();
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::RangeCheck,
        computation_resources
            .range_check_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::Pedersen,
        computation_resources
            .pedersen_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::Poseidon,
        computation_resources
            .poseidon_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::EcOp,
        computation_resources
            .ec_op_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::Ecdsa,
        computation_resources
            .ecdsa_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::Bitwise,
        computation_resources
            .bitwise_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::Keccak,
        computation_resources
            .keccak_builtin_applications
            .unwrap_or_default(),
    );
    builtin_instance_counter.insert(
        starknet_api::execution_resources::Builtin::SegmentArena,
        computation_resources
            .segment_arena_builtin
            .unwrap_or_default(),
    );
    builtin_instance_counter
}

/// This function converts from a vector of [`starknet_core::types::Event`] to a
/// vector of [`starknet_api::transaction::Event`].
///
/// # Arguments
///
/// - `input`: The input events.
///
/// # Errors
///
/// Returns [`Err`] if a [`starknet_core::types::Felt`] address is not a valid
/// [`starknet_api::core::ContractAddress`].
fn generate_events(
    input: Vec<starknet_core::types::Event>,
) -> Result<Vec<starknet_api::transaction::Event>, RpcClientError> {
    let mut events: Vec<starknet_api::transaction::Event> = Vec::with_capacity(input.len());
    input
        .into_iter()
        .try_for_each(|e| -> Result<(), RpcClientError> {
            events.push(Event {
                from_address: ContractAddress(e.from_address.try_into()?),
                content: EventContent {
                    keys: e.keys.into_iter().map(EventKey).collect(),
                    data: EventData(e.data),
                },
            });
            Ok(())
        })?;
    Ok(events)
}

/// This function converts from a vector of
/// [`starknet_core::types::MsgToL1`] to a vector of
/// [`starknet_api::transaction::MessageToL1`].
///
/// # Arguments
///
/// - `input`: The input messages.
///
/// # Errors
///
/// Returns [`Err`] if a [`starknet_core::types::Felt`] address is not a valid
/// [`starknet_api::core::ContractAddress`].
fn generate_messages(input: Vec<MsgToL1>) -> Result<Vec<MessageToL1>, RpcClientError> {
    let mut messages: Vec<MessageToL1> = Vec::with_capacity(input.len());
    input
        .into_iter()
        .try_for_each(|m| -> Result<(), RpcClientError> {
            messages.push(MessageToL1 {
                from_address: ContractAddress(m.from_address.try_into()?),
                to_address: {
                    let bytes = m.to_address.to_bytes_be();
                    let (_, h160_bytes) = bytes.split_at(12);
                    EthAddress(H160::from_slice(h160_bytes))
                },
                payload: L2ToL1Payload(m.payload),
            });
            Ok(())
        })?;
    Ok(messages)
}

/// This function converts [`starknet_core::types::ExecutionResources`] into
/// [`starknet_api::execution_resources::ExecutionResources`].
///
/// # Arguments
///
/// - `execution_resources`: The input object.
fn generate_execution_resources(
    execution_resources: &starknet_core::types::ExecutionResources,
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
            l1_gas: execution_resources
                .data_resources
                .data_availability
                .l1_gas
                .into(),
            l1_data_gas: execution_resources
                .data_resources
                .data_availability
                .l1_data_gas
                .into(),
            l2_gas: 0_u32.into(), // Not available in the RPC 0.7 protocol.
        },
        gas_consumed: GasVector {
            l1_gas: 0_u32.into(), // Not available in the RPC 0.7 protocol.
            l1_data_gas: execution_resources
                .data_resources
                .data_availability
                .l1_data_gas
                .into(),
            l2_gas: 0_u32.into(), // Not available in the RPC 0.7 protocol.
        },
    }
}

/// This function converts
/// [`starknet_core::types::ExecutionResult`] into
/// [`starknet_api::transaction::TransactionExecutionStatus`].
///
/// # Arguments
///
/// - `execution_result`: The input object.
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

/// This function converts [`starknet_core::types::TransactionReceipt`] into
/// [`starknet_api::transaction::TransactionReceipt`].
///
/// # Arguments
///
/// - `block_hash`: Hash of the block including the transaction.
/// - `block_number`: Number of the block including the transaction.
/// - `receipt`: The transaction receipt.
///
/// # Errors
///
/// Returns [`Err`] if `receipt` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::TransactionReceipt`] object.
pub fn convert_receipt(
    block_hash: &Felt,
    block_number: &u64,
    receipt: StarknetCoreReceipt,
) -> Result<StarknetApiReceipt, RpcClientError> {
    let block_hash = BlockHash(Felt::from_bytes_be(&block_hash.to_bytes_be()));
    let block_number = starknet_api::block::BlockNumber(*block_number);
    match receipt {
        StarknetCoreReceipt::Invoke(receipt) => {
            let tx_output = InvokeTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse()?),
                messages_sent: generate_messages(receipt.messages_sent)?,
                events: generate_events(receipt.events)?,
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(&receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash,
                block_number,
                output: TransactionOutput::Invoke(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::L1Handler(receipt) => {
            let tx_output = L1HandlerTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse()?),
                messages_sent: generate_messages(receipt.messages_sent)?,
                events: generate_events(receipt.events)?,
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(&receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash,
                block_number,
                output: TransactionOutput::L1Handler(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::Declare(receipt) => {
            let tx_output = DeclareTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse()?),
                messages_sent: generate_messages(receipt.messages_sent)?,
                events: generate_events(receipt.events)?,
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(&receipt.execution_resources),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash,
                block_number,
                output: TransactionOutput::Declare(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::Deploy(receipt) => {
            let tx_output = DeployTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse()?),
                messages_sent: generate_messages(receipt.messages_sent)?,
                events: generate_events(receipt.events)?,
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(&receipt.execution_resources),
                contract_address: ContractAddress(receipt.contract_address.try_into()?),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash,
                block_number,
                output: TransactionOutput::Deploy(tx_output),
            };
            Ok(receipt)
        }
        StarknetCoreReceipt::DeployAccount(receipt) => {
            let tx_output = DeployAccountTransactionOutput {
                actual_fee: Fee(receipt.actual_fee.amount.to_string().parse()?),
                messages_sent: generate_messages(receipt.messages_sent)?,
                events: generate_events(receipt.events)?,
                execution_status: generate_execution_status(receipt.execution_result),
                execution_resources: generate_execution_resources(&receipt.execution_resources),
                contract_address: ContractAddress(receipt.contract_address.try_into()?),
            };
            let receipt = StarknetApiReceipt {
                transaction_hash: TransactionHash(receipt.transaction_hash),
                block_hash,
                block_number,
                output: TransactionOutput::DeployAccount(tx_output),
            };
            Ok(receipt)
        }
    }
}
