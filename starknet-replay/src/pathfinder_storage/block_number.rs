//! This module contains the conversion between
//! [`pathfinder_common::BlockNumber`] and [`crate::common::BlockNumber`]

use pathfinder_common::BlockNumber as PathfinderBlockNumber;

use crate::common::BlockNumber;

impl From<PathfinderBlockNumber> for BlockNumber {
    fn from(item: PathfinderBlockNumber) -> Self {
        BlockNumber::new(item.get())
    }
}
impl From<BlockNumber> for PathfinderBlockNumber {
    fn from(val: BlockNumber) -> Self {
        // `new_or_panic` is acceptable because there is no casting of integers.
        PathfinderBlockNumber::new_or_panic(val.get())
    }
}
