use std::collections::HashMap;

use pathfinder_common::BlockNumber;
use starknet_api::core::ClassHash as StarknetClassHash;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ReplayClassHash {
    pub block_number: BlockNumber,
    pub class_hash: StarknetClassHash,
}

pub type VisitedPcs = HashMap<ReplayClassHash, Vec<Vec<usize>>>;
