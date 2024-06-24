use starknet_api::transaction::{
    DeclareTransaction,
    DeployAccountTransaction,
    InvokeTransaction,
    Transaction,
};

pub fn deserialize_transaction_json(
    transaction: &serde_json::Value,
) -> serde_json::Result<Transaction> {
    let mut transaction = transaction.clone();
    if let Some(resource_bounds) = transaction.get_mut("resource_bounds") {
        if let Some(l1_gas) = resource_bounds.get_mut("l1_gas") {
            resource_bounds["L1_GAS"] = l1_gas.clone();
            resource_bounds.as_object_mut().unwrap().remove("l1_gas");
        }
        if let Some(l2_gas) = resource_bounds.get_mut("l2_gas") {
            resource_bounds["L2_GAS"] = l2_gas.clone();
            resource_bounds.as_object_mut().unwrap().remove("l2_gas");
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
            x => Err(serde::de::Error::custom(format!(
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
            x => Err(serde::de::Error::custom(format!(
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
            x => Err(serde::de::Error::custom(format!(
                "unimplemented declare version: {x}"
            ))),
        },
        "L1_HANDLER" => Ok(Transaction::L1Handler(serde_json::from_value(transaction)?)),
        x => Err(serde::de::Error::custom(format!(
            "unimplemented transaction type deserialization: {x}"
        ))),
    }
}
