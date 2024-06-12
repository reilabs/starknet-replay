//! This module contains the definition of the struct [`ReplayClassHash`].

use std::collections::HashMap;

use starknet_api::core::ClassHash as StarknetClassHash;

use crate::runner::BlockNumber;

/// [`ReplayClassHash`] combines [`StarknetClassHash`] with
/// [`crate::block_number::BlockNumber`] in order to uniquely identify a
/// Contract Class from the database.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ReplayClassHash {
    /// The block number.
    pub block_number: BlockNumber,

    /// The class hash.
    pub class_hash: StarknetClassHash,
}

/// The type [`VisitedPcs`] is a hashmap to store the visited program counters
/// for each contract invocation during replay.
pub type VisitedPcs = HashMap<ReplayClassHash, Vec<Vec<usize>>>;
