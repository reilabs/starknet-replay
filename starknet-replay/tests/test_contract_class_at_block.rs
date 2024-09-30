//! The goal of this test is to query the `ClassDefinition` of a Starknet
//! contract from the RPC node. The input data shall be the `ClassHash`
//! and the block number. The test succeeds if the call to function
//! `get_contract_class_at_block` returns the expected `ClassDefinition`
//! object.

#![cfg(test)]

use std::{env, fs, io};

use itertools::Itertools;
use starknet_api::core::{ClassHash as StarknetClassHash, ClassHash};
use starknet_core::types::{ContractClass, Felt};
use starknet_replay::block_number::BlockNumber;
use starknet_replay::runner::replay_class_hash::ReplayClassHash;
use starknet_replay::storage::rpc::RpcStorage;
use starknet_replay::storage::Storage;
use url::Url;

fn read_test_file(filename: &str) -> io::Result<String> {
    let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let sierra_program_json_file = [out_dir.as_str(), filename].iter().join("");
    let sierra_program_json_file = sierra_program_json_file.as_str();
    fs::read_to_string(sierra_program_json_file)
}

#[test]
fn test_contract_class_at_block() {
    let block_number = BlockNumber::new(632917);
    let class_hash = "029927C8AF6BCCF3F6FDA035981E765A7BDBF18A2DC0D630494F8758AA908E2B";
    let class_hash: StarknetClassHash = ClassHash(Felt::from_hex(class_hash).unwrap());
    let replay_class_hash = ReplayClassHash {
        block_number,
        class_hash,
    };

    let endpoint: Url = Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
    let storage = RpcStorage::new(endpoint);
    let contract_class = storage
        .get_contract_class_at_block(&replay_class_hash)
        .unwrap();

    let sierra_program_json_file = "/test_data/test_contract_class_at_block.json";
    let sierra_program_json = read_test_file(sierra_program_json_file)
        .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_json_file}"));
    let sierra_program_json: serde_json::Value = serde_json::from_str(&sierra_program_json)
        .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_json_file} to json"));
    let contract_class_expected: ContractClass =
        serde_json::from_value::<ContractClass>(sierra_program_json).unwrap_or_else(|_| {
            panic!("Unable to parse {sierra_program_json_file} to SierraContractClass")
        });

    match (contract_class, contract_class_expected) {
        (ContractClass::Sierra(contract_class), ContractClass::Sierra(contract_class_expected)) => {
            assert_eq!(
                contract_class.sierra_program,
                contract_class_expected.sierra_program
            );

            assert_eq!(
                serde_json::to_value(contract_class.entry_points_by_type).unwrap(),
                serde_json::to_value(contract_class_expected.entry_points_by_type).unwrap()
            );
        }
        _ => panic!("Test failed, both contracts should be Sierra contracts"),
    };
}
