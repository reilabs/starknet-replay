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

pub fn deserialize_receipt_json(
    block: &serde_json::Value,
    receipt: &serde_json::Value,
) -> serde_json::Result<TransactionReceipt> {
    let mut receipt = receipt.clone();
    let block_number: BlockNumber = serde_json::from_value(block["block_number"].clone()).unwrap();
    let block_hash: BlockHash = serde_json::from_value(block["block_hash"].clone()).unwrap();
    let transaction_hash: TransactionHash =
        serde_json::from_value(receipt["transaction_hash"].clone()).unwrap();

    if let Some(actual_fee) = receipt.get_mut("actual_fee") {
        let fee = actual_fee["amount"].clone();
        receipt.as_object_mut().unwrap().remove("actual_fee");
        receipt["actual_fee"] = fee;
    }

    let mut builtin_instance_counter = HashMap::new();
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "range_check_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "pedersen_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "poseidon_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "ec_op_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "ecdsa_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "bitwise_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "keccak_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &mut receipt["execution_resources"],
        "segment_arena_builtin",
    );
    receipt["execution_resources"]["builtin_instance_counter"] =
        serde_json::to_value(builtin_instance_counter).unwrap();

    if receipt["execution_resources"].get("memory_holes").is_none() {
        receipt["execution_resources"]["memory_holes"] = serde_json::to_value(0).unwrap();
    }

    if let Some(execution_resources) = receipt.get_mut("execution_resources") {
        let l1_data_gas = execution_resources["data_availability"]["l1_data_gas"].clone();
        let l1_gas = execution_resources["data_availability"]["l1_gas"].clone();
        receipt["execution_resources"]
            .as_object_mut()
            .unwrap()
            .remove("data_availability");
        receipt["execution_resources"]["da_l1_gas_consumed"] = l1_gas;
        receipt["execution_resources"]["da_l1_data_gas_consumed"] = l1_data_gas;
    }

    let receipt_type: String = serde_json::from_value(receipt["type"].clone())?;
    match receipt_type.as_str() {
        "INVOKE" => {
            let receipt: InvokeTransactionOutput = serde_json::from_value(receipt).unwrap();
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Invoke(receipt),
            })
        }
        "DEPLOY_ACCOUNT" => {
            let receipt: DeployTransactionOutput = serde_json::from_value(receipt).unwrap();
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Deploy(receipt),
            })
        }
        "DECLARE" => {
            let receipt: DeclareTransactionOutput = serde_json::from_value(receipt).unwrap();
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::Declare(receipt),
            })
        }
        "L1_HANDLER" => {
            let receipt: L1HandlerTransactionOutput = serde_json::from_value(receipt).unwrap();
            Ok(TransactionReceipt {
                transaction_hash,
                block_hash,
                block_number,
                output: TransactionOutput::L1Handler(receipt),
            })
        }
        x => Err(serde::de::Error::custom(format!(
            "unimplemented transaction type deserialization: {x}"
        ))),
    }
}

fn add_builtin(map: &mut HashMap<Builtin, u64>, value: &mut serde_json::Value, builtin_name: &str) {
    if let Some(builtin_calls) = value.get(builtin_name) {
        let k = serde_json::from_value(builtin_name.into()).unwrap();
        let v = serde_json::from_value(builtin_calls.clone()).unwrap();
        map.insert(k, v);
    }
    value.as_object_mut().unwrap().remove(builtin_name);
}
