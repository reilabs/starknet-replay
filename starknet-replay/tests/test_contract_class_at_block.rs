//! The goal of this test is to query the `ClassDefinition` of a Starknet
//! contract to the Pathfinder database. The input data shall be the `ClassHash`
//! and the block number. The test succeeds if the call to function
//! `get_contract_class_at_block` returns the expected `ClassDefinition`
//! object.

#![cfg(test)]

use std::path::PathBuf;
use std::{env, fs, io};

use itertools::Itertools;
use pathfinder_rpc::v02::types::{ContractClass, SierraContractClass};
use starknet_api::core::ClassHash as StarknetClassHash;
use starknet_replay::block_number::BlockNumber;
use starknet_replay::runner::replay_class_hash::ReplayClassHash;
use starknet_replay::storage::pathfinder::PathfinderStorage;
use starknet_replay::storage::Storage;

fn read_test_file(filename: &str) -> io::Result<String> {
    let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let sierra_program_json_file = [out_dir.as_str(), filename].iter().join("");
    let sierra_program_json_file = sierra_program_json_file.as_str();
    fs::read_to_string(sierra_program_json_file)
}

// Ignored because it requires an updated copy of the pathfinder sqlite
// database.
#[ignore]
#[test]
fn test_contract_class_at_block() {
    let block_number = BlockNumber::new(632917);
    let class_hash = "029927C8AF6BCCF3F6FDA035981E765A7BDBF18A2DC0D630494F8758AA908E2B";
    let class_hash: StarknetClassHash = StarknetClassHash(class_hash.try_into().unwrap());
    let replay_class_hash = ReplayClassHash {
        block_number,
        class_hash,
    };

    let database_path = "../../pathfinder/mainnet.sqlite";
    let database_path = PathBuf::from(database_path);
    let storage = PathfinderStorage::new(database_path).unwrap();
    let contract_class = storage
        .get_contract_class_at_block(&replay_class_hash)
        .unwrap();
    let ContractClass::Sierra(contract_class) = contract_class else {
        panic!();
    };

    let sierra_program_json_file = "/test_data/test_contract_class_at_block.json";
    let sierra_program_json = read_test_file(sierra_program_json_file)
        .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_json_file}"));
    let sierra_program_json: serde_json::Value = serde_json::from_str(&sierra_program_json)
        .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_json_file} to json"));
    let contract_class_expected: SierraContractClass =
        serde_json::from_value::<SierraContractClass>(sierra_program_json).unwrap_or_else(|_| {
            panic!("Unable to parse {sierra_program_json_file} to SierraContractClass")
        });

    assert_eq!(
        contract_class.sierra_program,
        contract_class_expected.sierra_program
    );

    assert_eq!(
        contract_class.entry_points_by_type,
        contract_class_expected.entry_points_by_type
    );
}
