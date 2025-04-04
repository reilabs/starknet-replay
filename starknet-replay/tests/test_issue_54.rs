//! The goal of this test is to replay a single transaction, extract libfunc
//! statistics and verify that no stack overflow occurs.

#![cfg(test)]

use std::fs;

use starknet_api::hash::StarkHash;
use starknet_api::transaction::TransactionHash;
use starknet_core::types::Felt;
use starknet_replay::block_number::BlockNumber;
use starknet_replay::profiler::analysis::extract_libfuncs_weight;
use starknet_replay::profiler::replay_statistics::ReplayStatistics;
use starknet_replay::runner::replay_block::ReplayBlock;
use starknet_replay::runner::replay_blocks_parallel;
use starknet_replay::storage::rpc::RpcStorage;
use starknet_replay::storage::Storage;
use test_log::test;
use url::Url;

#[test]
fn test_issue_54() {
    let block_number = 900000;
    let transaction_hash: StarkHash =
        Felt::from_hex("0x0108C7451D3C09EF2E7F1CC6541375E6FA0838479DA435AAAD65C6E09BFD622B")
            .unwrap();

    let endpoint: Url = Url::parse("https://starknet-mainnet.public.blastapi.io/rpc/v0_7").unwrap();
    let read_from_state = false;
    let storage = RpcStorage::new(endpoint, read_from_state);
    let mut replay_work: Vec<ReplayBlock> = Vec::new();

    let block_number = BlockNumber::new(block_number);

    let (block_header, transactions, receipts) = storage
        .get_transactions_and_receipts_for_block(block_number)
        .unwrap();

    let index = receipts
        .iter()
        .position(|r| r.transaction_hash == TransactionHash(transaction_hash))
        .unwrap();

    let transactions = vec![transactions.get(index).unwrap().clone()];
    let receipts = vec![receipts.get(index).unwrap().clone()];

    let replay_block = ReplayBlock::new(block_header, transactions, receipts).unwrap();
    replay_work.push(replay_block);

    let trace_out = None;
    let visited_pcs = replay_blocks_parallel(&storage, &trace_out, &replay_work).unwrap();

    let libfunc_stats = extract_libfuncs_weight(&visited_pcs, &storage).unwrap();

    let mut replay_statistics_expected = ReplayStatistics::new();
    let contents = fs::read_to_string("./test_data/test_issue_54.out").unwrap(); // skipping 1 line for header
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
