//! This module contains the methods to manipulate Starknet contract classes.

use std::io::Read;

use blockifier::execution::contract_class::{
    ContractClass as BlockifierContractClass,
    ContractClassV0,
    ContractClassV1,
};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use flate2::bufread;
use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
use starknet::core::types::{CompressedLegacyContractClass, FlattenedSierraClass};
use starknet_api::deprecated_contract_class::ContractClass as DeprecatedContractClass;

use crate::error::DatabaseError;
use crate::storage::rpc::contract_class;

/// This function converts [`starknet::core::types::FlattenedSierraClass`]
/// into [`blockifier::execution::contract_class::ContractClass`].
///
/// # Arguments
///
/// - `input`: The compressesed Sierra program.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - Serialisation of `input` fails.
/// - Compilation of Sierra into CASM fals.
pub fn decompress_sierra(
    input: FlattenedSierraClass,
) -> Result<BlockifierContractClass, DatabaseError> {
    let mut contract_class = serde_json::to_value(input)?;
    contract_class
        .as_object_mut()
        .ok_or(DatabaseError::Unknown(
            "Contract class is not an object".to_string(),
        ))?
        .remove("abi");
    let sierra_cc: CairoContractClass = serde_json::from_value(contract_class)?;
    let casm_definition = CasmContractClass::from_contract_class(sierra_cc, false, usize::MAX)?;
    let contract_class: ContractClassV1 = casm_definition.try_into().map_err(|_| {
        DatabaseError::IntoInvalid(
            "CasmContractClass".to_string(),
            "ContractClassV1".to_string(),
        )
    })?;
    let contract_class = BlockifierContractClass::V1(contract_class);
    Ok(contract_class)
}

/// This function converts
/// [`starknet::core::types::CompressedLegacyContractClass`]
/// into [`blockifier::execution::contract_class::ContractClass`].
///
/// # Arguments
///
/// - `input`: The compressesed CASM program.
///
/// # Errors
///
/// Returns [`Err`] if serialisation of `input` fails.
pub fn decompress_casm(
    input: CompressedLegacyContractClass,
) -> Result<BlockifierContractClass, DatabaseError> {
    let contract_class = contract_class::decompress_casm_with_abi(input)?;
    let contract_class = serde_json::to_value(contract_class)?;
    let contract_class: DeprecatedContractClass = serde_json::from_value(contract_class)?;
    let contract_class: ContractClassV0 = contract_class.try_into().map_err(|_| {
        DatabaseError::IntoInvalid(
            "DeprecatedContractClass".to_string(),
            "ContractClassV0".to_string(),
        )
    })?;
    Ok(BlockifierContractClass::V0(contract_class))
}

/// Decompress a contract class.
///
/// This function is useful because the RPC protocol transmits compressed
/// contract classes.
///
/// # Arguments
///
/// - `input`: The compressesed CASM contract class.
///
/// # Errors
///
/// Returns [`Err`] if `input` is not a valid compressed contract class.
pub fn decompress_casm_with_abi(
    input: CompressedLegacyContractClass,
) -> Result<LegacyContractClass, DatabaseError> {
    let mut gz = bufread::GzDecoder::new(&input.program[..]);
    let mut decoded_program = String::new();
    gz.read_to_string(&mut decoded_program)?;
    let decoded_program: LegacyProgram = serde_json::from_str(&decoded_program)?;
    let entry_points = serde_json::to_value(input.entry_points_by_type)?;
    let abi = serde_json::to_value(input.abi)?;
    let contract_class = LegacyContractClass {
        abi: serde_json::from_value(abi)?,
        entry_points_by_type: serde_json::from_value(entry_points)?,
        program: decoded_program,
    };
    Ok(contract_class)
}
