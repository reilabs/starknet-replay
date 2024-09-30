use starknet_api::core::PatriciaKey;
use starknet_core::types::Felt;

/// Returns `Felt` from `PatriciaKey` element.
///
/// # Arguments
///
/// - `contract_address`: The contract address.
#[must_use]
pub fn to_field_element(contract_address: &PatriciaKey) -> Felt {
    **contract_address
}
