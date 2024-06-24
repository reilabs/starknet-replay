use std::io::Read;

use flate2::bufread;
use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
use starknet::core::types::CompressedLegacyContractClass;

pub fn decompress(input: CompressedLegacyContractClass) -> LegacyContractClass {
    let mut gz = bufread::GzDecoder::new(&input.program[..]);
    let mut decoded_program = String::new();
    gz.read_to_string(&mut decoded_program).unwrap();
    let decoded_program: LegacyProgram = serde_json::from_str(&decoded_program).unwrap();
    let entry_points = serde_json::to_value(input.entry_points_by_type).unwrap();
    let abi = serde_json::to_value(input.abi).unwrap();
    LegacyContractClass {
        abi: serde_json::from_value(abi).unwrap(),
        entry_points_by_type: serde_json::from_value(entry_points).unwrap(),
        program: decoded_program,
    }
}
