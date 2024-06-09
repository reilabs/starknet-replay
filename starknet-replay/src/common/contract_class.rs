use std::collections::HashMap;

use cairo_lang_utils::bigint::BigUintAsHex;
use serde::{Deserialize, Serialize};
use starknet_api::state::{EntryPoint, EntryPointType};

type Felt = BigUintAsHex;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractClass {
    #[serde(rename = "sierra_program")]
    pub compressed_sierra_program: Vec<Felt>,
    pub contract_class_version: String,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPoint>>,
    pub abi: String,
}
