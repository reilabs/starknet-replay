//! The goal of this test is to replay a single transaction, extract libfunc
//! statistics and verify the results are as expected.

#![cfg(test)]

// Ignored because it requires an updated copy of the pathfinder sqlite
// database.
#[ignore]
#[test]
fn test_replay_blocks() {
    // No trait is available to mock the `PathfinderStorage` struct.
    // Issue #9.
    // Need to make a version for Papyrus node.
}
