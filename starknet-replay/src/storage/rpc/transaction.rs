//! This module is needed to deserialise all the transaction receipts returned
//! with `starknet_getBlockWithReceipts` into
//! [`starknet_api::transaction::Transaction`].

use starknet_api::transaction::{
    DeclareTransaction,
    DeployAccountTransaction,
    InvokeTransaction,
    Transaction,
};

use crate::error::DatabaseError;

/// This function returns a [`starknet_api::transaction::Transaction`]
/// from a serialised transaction.
///
/// # Arguments
///
/// - `transaction`: the transaction in JSON format. It shall be of the format
///   returned by `starknet_getBlockWithReceipts`.
///
/// # Errors
///
/// Returns [`Err`] if the JSON data is not in the correct format.
#[allow(clippy::too_many_lines)] // Added because there is a lot of repetition.
pub fn deserialize_transaction_json(
    transaction: &serde_json::Value,
) -> Result<Transaction, DatabaseError> {
    let mut transaction = transaction.clone();
    if let Some(resource_bounds) = transaction.get_mut("resource_bounds") {
        if let Some(l1_gas) = resource_bounds.get_mut("l1_gas") {
            resource_bounds["L1_GAS"] = l1_gas.clone();
            resource_bounds
                .as_object_mut()
                .ok_or(DatabaseError::Unknown(
                    "Failed to serialise transaction as object.".to_string(),
                ))?
                .remove("l1_gas");
        }
        if let Some(l2_gas) = resource_bounds.get_mut("l2_gas") {
            resource_bounds["L2_GAS"] = l2_gas.clone();
            resource_bounds
                .as_object_mut()
                .ok_or(DatabaseError::Unknown(
                    "Failed to serialise transaction as object.".to_string(),
                ))?
                .remove("l2_gas");
        }
    }

    let tx_type: String = serde_json::from_value(transaction["type"].clone())?;
    let tx_version: String = serde_json::from_value(transaction["version"].clone())?;

    match tx_type.as_str() {
        "INVOKE" => match tx_version.as_str() {
            "0x0" => Ok(Transaction::Invoke(InvokeTransaction::V0(
                serde_json::from_value(transaction)?,
            ))),
            "0x1" => Ok(Transaction::Invoke(InvokeTransaction::V1(
                serde_json::from_value(transaction)?,
            ))),
            "0x3" => Ok(Transaction::Invoke(InvokeTransaction::V3(
                serde_json::from_value(transaction)?,
            ))),
            x => Err(DatabaseError::Unknown(format!(
                "unimplemented invoke version: {x}"
            ))),
        },
        "DEPLOY_ACCOUNT" => match tx_version.as_str() {
            "0x1" => Ok(Transaction::DeployAccount(DeployAccountTransaction::V1(
                serde_json::from_value(transaction)?,
            ))),
            "0x3" => Ok(Transaction::DeployAccount(DeployAccountTransaction::V3(
                serde_json::from_value(transaction)?,
            ))),
            x => Err(DatabaseError::Unknown(format!(
                "unimplemented declare version: {x}"
            ))),
        },
        "DECLARE" => match tx_version.as_str() {
            "0x0" => Ok(Transaction::Declare(DeclareTransaction::V0(
                serde_json::from_value(transaction)?,
            ))),
            "0x1" => Ok(Transaction::Declare(DeclareTransaction::V1(
                serde_json::from_value(transaction)?,
            ))),
            "0x2" => Ok(Transaction::Declare(DeclareTransaction::V2(
                serde_json::from_value(transaction)?,
            ))),
            "0x3" => Ok(Transaction::Declare(DeclareTransaction::V3(
                serde_json::from_value(transaction)?,
            ))),
            x => Err(DatabaseError::Unknown(format!(
                "unimplemented declare version: {x}"
            ))),
        },
        "L1_HANDLER" => Ok(Transaction::L1Handler(serde_json::from_value(transaction)?)),
        x => Err(DatabaseError::Unknown(format!(
            "unimplemented transaction type deserialization: {x}"
        ))),
    }
}
