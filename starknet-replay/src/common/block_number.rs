use pathfinder_common::BlockNumber as PathfinderBlockNumber;

#[derive(Copy, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockNumber(u64);

impl From<PathfinderBlockNumber> for BlockNumber {
    fn from(item: PathfinderBlockNumber) -> Self {
        BlockNumber(item.get())
    }
}

// TODO: add `papyrus` block number
