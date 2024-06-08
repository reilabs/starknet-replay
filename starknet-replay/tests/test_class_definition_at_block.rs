//! The goal of this test is to query the `ClassDefinition` of a Starknet
//! contract to the Pathfinder database. The input data shall be the `ClassHash`
//! and the block number. The test succeeds if the call to function
//! `get_contract_class_at_block` returns the expected `ClassDefinition`
//! object.

#![cfg(test)]

#[ignore]
#[test]
fn test_class_definition_at_block() {
    // No trait is available to mock the `Transaction` struct.
    // May need to use a real `pathfinder` db.
    // Issue #9.
    assert!(true);
}
