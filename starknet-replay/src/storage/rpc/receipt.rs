//! This module is needed to deserialise all the transaction receipts returned
//! with `starknet_getBlockWithReceipts` into
//! [`starknet_api::transaction::TransactionReceipt`].

use std::collections::HashMap;

use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::transaction::{
    Builtin,
    DeclareTransactionOutput,
    DeployTransactionOutput,
    InvokeTransactionOutput,
    L1HandlerTransactionOutput,
    TransactionHash,
    TransactionOutput,
    TransactionReceipt,
};

use crate::error::DatabaseError;

/// This function returns a [`starknet_api::transaction::TransactionReceipt`]
/// from a serialised receipt.
///
/// # Arguments
///
/// - `block`: the block header where the receipt is included. It shall be of
///   the format returned by `starknet_getBlockWithReceipts`.
/// - `receipt`: the receipt in JSON format. It shall be of the format returned
///   by `starknet_getBlockWithReceipts`.
///
/// # Errors
///
/// Returns [`Err`] if the JSON data is not in the correct format.
#[allow(clippy::too_many_lines)] // Added because there is a lot of repetition.
pub fn deserialize_receipt_json(
    block: &serde_json::Value,
    receipt: &serde_json::Value,
) -> Result<TransactionReceipt, DatabaseError> {
    let mut receipt = receipt.clone();
    let block_number: BlockNumber = serde_json::from_value(block["block_number"].clone())?;
    let block_hash: BlockHash = serde_json::from_value(block["block_hash"].clone())?;
    let transaction_hash: TransactionHash =
        serde_json::from_value(receipt["transaction_hash"].clone())?;

    if let Some(actual_fee) = receipt.get_mut("actual_fee") {
        let fee = actual_fee["amount"].clone();
        receipt
            .as_object_mut()
            .ok_or(DatabaseError::Unknown(
                "Failed to serialise transaction receipt as object.".to_string(),
            ))?
            .remove("actual_fee");
        receipt["actual_fee"] = fee;
    }

    let mut builtin_instance_counter = HashMap::new();
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "range_check_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "pedersen_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "poseidon_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "ec_op_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "ecdsa_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "bitwise_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "keccak_builtin_applications",
    )?;
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "segment_arena_builtin",
    )?;
    receipt["execution_resources"]["builtin_instance_counter"] =
        serde_json::to_value(builtin_instance_counter)?;

    if receipt["execution_resources"].get("memory_holes").is_none() {
        receipt["execution_resources"]["memory_holes"] = serde_json::to_value(0)?;
    }

    if let Some(execution_resources) = receipt.get_mut("execution_resources") {
        let mut l1_data_gas = execution_resources["data_availability"]["l1_data_gas"].clone();
        if l1_data_gas.is_null() {
            // Very old blocks report null
            l1_data_gas = 0.into();
        }
        let mut l1_gas = execution_resources["data_availability"]["l1_gas"].clone();
        if l1_gas.is_null() {
            // Very old blocks report null
            l1_gas = 0.into();
        }
        receipt["execution_resources"]
            .as_object_mut()
            .ok_or(DatabaseError::Unknown(
                "Failed to serialise transaction receipt as object.".to_string(),
            ))?
            .remove("data_availability");
        receipt["execution_resources"]["da_l1_gas_consumed"] = l1_gas;
        receipt["execution_resources"]["da_l1_data_gas_consumed"] = l1_data_gas;
    }

    let receipt_type: String = serde_json::from_value(receipt["type"].clone())?;
    match receipt_type.as_str() {
        "INVOKE" => {
            println!("{receipt:#?}");
            let receipt: InvokeTransactionOutput = serde_json::from_value(receipt)?;
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Invoke(receipt),
            })
        }
        "DEPLOY_ACCOUNT" => {
            let receipt: DeployTransactionOutput = serde_json::from_value(receipt)?;
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Deploy(receipt),
            })
        }
        "DECLARE" => {
            let receipt: DeclareTransactionOutput = serde_json::from_value(receipt)?;
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Declare(receipt),
            })
        }
        "L1_HANDLER" => {
            let receipt: L1HandlerTransactionOutput = serde_json::from_value(receipt)?;
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::L1Handler(receipt),
            })
        }
        x => Err(DatabaseError::Unknown(format!(
            "unimplemented transaction type deserialization: {x}"
        ))),
    }
}

/// This function formats the builtins for
/// [`starknet_api::transaction::TransactionReceipt`].
///
/// It is needed because the builtins are in a different format when received
/// from the RPC call.
///
/// # Arguments
///
/// - `map`: the map keeping track of the builtins called and the frequency.
/// - `value`: the list of builtins from the RPC call.
/// - `builtin_name`: the name of the builtin to add to `map`.
///
/// # Errors
///
/// Returns [`Err`] if the JSON data is not in the correct format or
/// `builtin_name` is missing from `value`.
fn add_builtin(
    map: &mut HashMap<Builtin, u64>,
    value: &mut serde_json::Value,
    builtin_name: &str,
) -> Result<(), DatabaseError> {
    if let Some(builtin_calls) = value.get(builtin_name) {
        let k = serde_json::from_value(builtin_name.into())?;
        let v = serde_json::from_value(builtin_calls.clone())?;
        map.insert(k, v);
    }
    value
        .as_object_mut()
        .ok_or(DatabaseError::Unknown(
            "Failed to serialise transaction receipt as object.".to_string(),
        ))?
        .remove(builtin_name);
    Ok(())
}
