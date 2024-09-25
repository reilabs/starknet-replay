use std::collections::{BTreeMap, HashMap};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::ops::Add;
use std::path::PathBuf;

use blockifier::execution::call_info::CallInfo;
use blockifier::transaction::objects::TransactionExecutionInfo;
use blockifier::transaction::transaction_types::TransactionType;
use cairo_vm::types::builtin_name::BuiltinName;
use starknet_core::types::{
    ComputationResources,
    DataAvailabilityResources,
    DataResources,
    DeclareTransactionTrace,
    DeployAccountTransactionTrace,
    ExecuteInvocation,
    ExecutionResources,
    Felt,
    FunctionInvocation,
    InvokeTransactionTrace,
    L1HandlerTransactionTrace,
    OrderedEvent,
    OrderedMessage,
    RevertedInvocation,
    StateDiff,
    TransactionTrace,
};

use crate::error::RunnerError;

/// This function returns the usage of a builtin from a hashmap of builtins.
///
/// If the builtin isn't present, it returns None.
/// This function assumes that `usize` is represented under the hood by less
/// than 64bit, otherwise the function retuns 0 on overflow when casting from
/// `usize` to `u64`.
///
/// # Arguments
///
/// - `builtin_map`: the hashmap mapping builtins to usage.
/// - `builtin`: the builtin usage to return.
fn get_builtin_usage(
    builtin_map: &HashMap<BuiltinName, usize>,
    builtin: BuiltinName,
) -> Option<u64> {
    builtin_map
        .get(&builtin)
        .map(|n| (*n).try_into().unwrap_or_default())
}

/// Returns a vector of [`starknet_core::types::OrderedMessage`] from
/// transaction call data.
///
/// # Arguments
///
/// - `call_info`: the transaction call data.
fn ordered_l2_to_l1_messages(
    call_info: &blockifier::execution::call_info::CallInfo,
) -> Vec<OrderedMessage> {
    let mut messages = BTreeMap::new();

    for call in call_info.iter() {
        for message in &call.execution.l2_to_l1_messages {
            messages.insert(
                message.order,
                OrderedMessage {
                    order: message.order.try_into().unwrap_or_default(),
                    payload: message.message.payload.0.clone(),
                    to_address: Felt::from_bytes_be_slice(message.message.to_address.0.as_bytes()),
                    from_address: *call.call.storage_address.0.key(),
                },
            );
        }
    }

    messages.into_values().collect()
}

/// This function generates a `FunctionInvocation` object from a `CallInfo`
/// object. The `FunctionInvocation` object is part of the transaction trace.
///
/// # Arguments
///
/// - `call_info`: The Starknet call to process.
fn generate_function_invocation(call_info: &CallInfo) -> FunctionInvocation {
    let resources = &call_info.resources;
    let computation_resources = ComputationResources {
        steps: resources.n_steps.try_into().unwrap_or_default(),
        memory_holes: Some(resources.n_memory_holes.try_into().unwrap_or_default()),
        range_check_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::range_check,
        ),
        pedersen_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::pedersen,
        ),
        poseidon_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::poseidon,
        ),
        ec_op_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::ec_op,
        ),
        ecdsa_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::ecdsa,
        ),
        bitwise_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::bitwise,
        ),
        keccak_builtin_applications: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::keccak,
        ),
        segment_arena_builtin: get_builtin_usage(
            &resources.builtin_instance_counter,
            BuiltinName::segment_arena,
        ),
    };
    FunctionInvocation {
        contract_address: *call_info.call.storage_address.0.key(),
        entry_point_selector: call_info.call.entry_point_selector.0,
        calldata: call_info.call.calldata.0.to_vec(),
        caller_address: *call_info.call.caller_address.0.key(),
        class_hash: *call_info.call.class_hash.unwrap_or_default(),
        entry_point_type: match call_info.call.entry_point_type {
            starknet_api::deprecated_contract_class::EntryPointType::Constructor => {
                starknet_core::types::EntryPointType::Constructor
            }
            starknet_api::deprecated_contract_class::EntryPointType::External => {
                starknet_core::types::EntryPointType::External
            }
            starknet_api::deprecated_contract_class::EntryPointType::L1Handler => {
                starknet_core::types::EntryPointType::L1Handler
            }
        },
        call_type: match call_info.call.call_type {
            blockifier::execution::entry_point::CallType::Call => {
                starknet_core::types::CallType::Call
            }
            blockifier::execution::entry_point::CallType::Delegate => {
                starknet_core::types::CallType::Delegate
            }
        },
        result: call_info.execution.retdata.0.clone(),
        calls: call_info
            .inner_calls
            .iter()
            .map(generate_function_invocation)
            .collect(),
        events: call_info
            .execution
            .events
            .iter()
            .map(|event| OrderedEvent {
                order: event.order.try_into().unwrap_or_default(),
                data: event.event.data.0.clone(),
                keys: event.event.keys.iter().map(|key| key.0).collect(),
            })
            .collect(),
        messages: ordered_l2_to_l1_messages(call_info),
        execution_resources: computation_resources,
    }
}

/// This function sums two options and returns the result in an option.
///
/// If one element is `None`, it is assumed as zero. If both elements are
/// `None`, the result is `None`.
///
/// # Arguments
///
/// - `lhs`: The left hand side element of the addition.
/// - `rhs`: The right hand side element of the addition.
fn sum_options<T: Add<Output = T>>(lhs: Option<T>, rhs: Option<T>) -> Option<T> {
    match (lhs, rhs) {
        (None, None) => None,
        (None, Some(b)) => Some(b),
        (Some(a), None) => Some(a),
        (Some(a), Some(b)) => Some(a + b),
    }
}

/// This function sums two `ComputationResources` objects by adding each field
/// of the structures together.
///
/// # Arguments
///
/// - `lhs`: The left hand side element of the addition.
/// - `rhs`: The right hand side element of the addition.
fn add(lhs: &ComputationResources, rhs: &ComputationResources) -> ComputationResources {
    ComputationResources {
        steps: lhs.steps + rhs.steps,
        memory_holes: sum_options(lhs.memory_holes, rhs.memory_holes),
        range_check_builtin_applications: sum_options(
            lhs.range_check_builtin_applications,
            rhs.range_check_builtin_applications,
        ),
        pedersen_builtin_applications: sum_options(
            lhs.pedersen_builtin_applications,
            rhs.pedersen_builtin_applications,
        ),
        poseidon_builtin_applications: sum_options(
            lhs.poseidon_builtin_applications,
            rhs.poseidon_builtin_applications,
        ),
        ec_op_builtin_applications: sum_options(
            lhs.ec_op_builtin_applications,
            rhs.ec_op_builtin_applications,
        ),
        ecdsa_builtin_applications: sum_options(
            lhs.ecdsa_builtin_applications,
            rhs.ecdsa_builtin_applications,
        ),
        bitwise_builtin_applications: sum_options(
            lhs.bitwise_builtin_applications,
            rhs.bitwise_builtin_applications,
        ),
        keccak_builtin_applications: sum_options(
            lhs.keccak_builtin_applications,
            rhs.keccak_builtin_applications,
        ),
        segment_arena_builtin: sum_options(lhs.segment_arena_builtin, rhs.segment_arena_builtin),
    }
}

/// This function implements the default initialisation of a
/// `ComputationResources` object because it's not already derived in the
/// `starknet_core` crate.
fn default_computation_resources() -> ComputationResources {
    ComputationResources {
        steps: 0,
        memory_holes: None,
        range_check_builtin_applications: None,
        pedersen_builtin_applications: None,
        poseidon_builtin_applications: None,
        ec_op_builtin_applications: None,
        ecdsa_builtin_applications: None,
        bitwise_builtin_applications: None,
        keccak_builtin_applications: None,
        segment_arena_builtin: None,
    }
}

/// This function builds a transaction trace from the data returned by the
/// transaction execution.
///
/// # Arguments
///
/// - `transaction_type`: The type of the transaction to determine which type of
///   transaction trace to generate.
/// - `execution_info`: The data from transaction execution.
/// - `state_diff`: The blockchain state changes from transaction execution.
fn create_transaction_trace(
    transaction_type: TransactionType,
    execution_info: &TransactionExecutionInfo,
    state_diff: Option<StateDiff>,
) -> TransactionTrace {
    let validate_invocation = execution_info
        .validate_call_info
        .as_ref()
        .map(generate_function_invocation);
    let maybe_function_invocation = execution_info
        .execute_call_info
        .as_ref()
        .map(generate_function_invocation);
    let fee_transfer_invocation = execution_info
        .fee_transfer_call_info
        .as_ref()
        .map(generate_function_invocation);

    let computation_resources = add(
        &validate_invocation
            .as_ref()
            .map_or(default_computation_resources(), |i: &FunctionInvocation| {
                i.execution_resources.clone()
            }),
        &maybe_function_invocation
            .as_ref()
            .map_or(default_computation_resources(), |i: &FunctionInvocation| {
                i.execution_resources.clone()
            }),
    );
    let computation_resources = add(
        &computation_resources,
        &fee_transfer_invocation
            .as_ref()
            .map_or(default_computation_resources(), |i: &FunctionInvocation| {
                i.execution_resources.clone()
            }),
    );
    let data_resources = DataResources {
        data_availability: DataAvailabilityResources {
            l1_gas: execution_info
                .transaction_receipt
                .da_gas
                .l1_gas
                .try_into()
                .unwrap_or_default(),
            l1_data_gas: execution_info
                .transaction_receipt
                .da_gas
                .l1_data_gas
                .try_into()
                .unwrap_or_default(),
        },
    };
    let execution_resources = ExecutionResources {
        computation_resources,
        data_resources,
    };

    match transaction_type {
        TransactionType::Declare => TransactionTrace::Declare(DeclareTransactionTrace {
            validate_invocation,
            fee_transfer_invocation,
            state_diff,
            execution_resources,
        }),
        TransactionType::DeployAccount => {
            TransactionTrace::DeployAccount(DeployAccountTransactionTrace {
                validate_invocation,
                constructor_invocation: maybe_function_invocation.expect("execute_call_info"),
                fee_transfer_invocation,
                state_diff,
                execution_resources,
            })
        }
        TransactionType::InvokeFunction => TransactionTrace::Invoke(InvokeTransactionTrace {
            validate_invocation,
            execute_invocation: if let Some(revert_reason) = &execution_info.revert_error {
                ExecuteInvocation::Reverted(RevertedInvocation {
                    revert_reason: revert_reason.to_string(),
                })
            } else {
                ExecuteInvocation::Success(maybe_function_invocation.expect("execute_call_info"))
            },
            fee_transfer_invocation,
            state_diff,
            execution_resources,
        }),
        TransactionType::L1Handler => TransactionTrace::L1Handler(L1HandlerTransactionTrace {
            function_invocation: maybe_function_invocation.expect("execute_call_info"),
            state_diff,
            execution_resources,
        }),
    }
}

/// Writes transaction traces to JSON file.
///
/// Transaction traces are appended to the file. There is a trace per line.
///
/// # Arguments
///
/// - `filename`: The file to output the trace.
/// - `execution_info`: The data from transaction execution.
/// - `transaction_type`: The type of the transaction to determine which type of
///   transaction trace to generate.
/// - `state_diff`: The blockchain state changes from transaction execution.
///
/// # Errors
///
/// Returns [`Err`] if there is any error writing to `filename`.
pub fn write_to_file(
    filename: &PathBuf,
    execution_info: &TransactionExecutionInfo,
    transaction_type: TransactionType,
    state_diff: Option<StateDiff>,
) -> Result<(), RunnerError> {
    let output_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)?;
    let mut f = BufWriter::new(output_file);
    let transaction_trace = create_transaction_trace(transaction_type, execution_info, state_diff);
    let output = serde_json::to_string(&transaction_trace)?;
    f.write_all(output.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}
