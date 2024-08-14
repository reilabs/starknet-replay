use starknet_api::core::PatriciaKey;
use starknet_core::types::Felt;

use crate::error::DatabaseError;

pub fn to_field_element(contract_address: &PatriciaKey) -> Result<Felt, DatabaseError> {
    Ok(**contract_address)
    // let field_element =
    //     FieldElement::from_bytes_be(contract_address.bytes().try_into().
    // unwrap()).unwrap(); Ok(field_element)
}
