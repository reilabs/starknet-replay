use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use starknet_api::transaction::{Builtin, ExecutionResources};

#[derive(Debug, Deserialize)]
pub struct RpcTransactionReceipt {
    pub actual_fee: FeePayment,
    pub execution_status: String,
    #[serde(rename = "type")]
    pub tx_type: String,
    #[serde(deserialize_with = "vm_execution_resources_deser")]
    pub execution_resources: ExecutionResources,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
pub struct FeePayment {
    #[serde(deserialize_with = "fee_amount_deser")]
    pub amount: u128,
    pub unit: String,
}

fn fee_amount_deser<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let hex: String = Deserialize::deserialize(deserializer)?;
    u128::from_str_radix(&hex[2..], 16).map_err(serde::de::Error::custom)
}

fn add_builtin(map: &mut HashMap<Builtin, u64>, value: &serde_json::Value, builtin_name: &str) {
    if let Some(builtin_calls) = value.get(builtin_name) {
        let k = serde_json::from_value(builtin_name.into()).unwrap();
        let v = serde_json::from_value(builtin_calls.clone()).unwrap();
        map.insert(k, v);
    }
}

fn vm_execution_resources_deser<'de, D>(deserializer: D) -> Result<ExecutionResources, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    // Parse n_steps
    let steps: u64 = serde_json::from_value(
        value
            .get("steps")
            .ok_or(serde::de::Error::custom(
                "Missing steps field", /* RpcStateError::MissingRpcResponseField("steps".
                                        * to_string()), */
            ))?
            .clone(),
    )
    .map_err(|e| serde::de::Error::custom(e.to_string()))?;

    // Parse n_memory_holes
    let memory_holes: u64 = if let Some(memory_holes) = value.get("memory_holes") {
        serde_json::from_value(memory_holes.clone())
            .map_err(|e| serde::de::Error::custom(e.to_string()))?
    } else {
        0
    };

    // Parse builtin instance counter
    let mut builtin_instance_counter = HashMap::new();

    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "range_check_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "pedersen_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "poseidon_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "ec_op_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "ecdsa_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "bitwise_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "keccak_builtin_applications",
    );
    add_builtin(
        &mut builtin_instance_counter,
        &value,
        "segment_arena_builtin",
    );

    Ok(ExecutionResources {
        builtin_instance_counter,
        steps,
        memory_holes,
        da_l1_gas_consumed: 0,
        da_l1_data_gas_consumed: 0,
    })
}
