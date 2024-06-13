//! The goal of this test is to replay a single transaction, extract libfunc
//! statistics and verify the results are as expected.

#![cfg(test)]

use std::fs;
use std::path::PathBuf;

use starknet_replay::block_number::BlockNumber;
use starknet_replay::profiler::analysis::extract_libfuncs_weight;
use starknet_replay::profiler::replay_statistics::ReplayStatistics;
use starknet_replay::runner::replay_block::ReplayBlock;
use starknet_replay::runner::replay_blocks;
use starknet_replay::storage::pathfinder::PathfinderStorage;
use starknet_replay::storage::Storage;

// Ignored because it requires an updated copy of the pathfinder sqlite
// database.
#[ignore]
#[test]
fn test_replay_blocks() {
    let database_path = "../../pathfinder/mainnet.sqlite";
    let block_number = 632917;
    let transaction_hash = "0x0177C9365875CAA840EA8F03F97B0E3A8EE8851A8B952BF157B5DBD4FECCB060";

    let database_path = PathBuf::from(database_path);
    let storage = PathfinderStorage::new(database_path).unwrap();
    let mut replay_work: Vec<ReplayBlock> = Vec::new();

    let block_number = BlockNumber::new(block_number);

    let (transactions, receipts) = storage
        .get_transactions_and_receipts_for_block(block_number)
        .unwrap();

    let index = receipts
        .iter()
        .position(|r| r.transaction_hash.to_string() == transaction_hash)
        .unwrap();

    let transactions = vec![transactions.get(index).unwrap().clone()];
    let receipts = vec![receipts.get(index).unwrap().clone()];

    let header = storage.get_block_header(block_number).unwrap();
    let replay_block = ReplayBlock::new(header, transactions, receipts).unwrap();
    replay_work.push(replay_block);

    let trace_out = None;
    let visited_pcs = replay_blocks(&storage.clone(), &trace_out, &replay_work).unwrap();

    let libfunc_stats = extract_libfuncs_weight(&visited_pcs, &storage).unwrap();

    let mut replay_statistics_expected = ReplayStatistics::new();
    let contents = fs::read_to_string("./test_data/test_replay_blocks.out").unwrap();
    // skipping 1 line for header
    for line in contents.lines().skip(1) {
        let line: Vec<&str> = line.split(',').collect();
        let libfunc_name = line.as_slice()[0..line.len() - 1].join(",");
        let Ok(frequency) = line.last().unwrap().parse::<usize>() else {
            continue;
        };
        replay_statistics_expected.update(&libfunc_name, frequency);
    }
    assert_eq!(libfunc_stats, replay_statistics_expected);
}
