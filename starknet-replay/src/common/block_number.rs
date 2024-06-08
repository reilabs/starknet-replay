use pathfinder_common::BlockNumber as PathfinderBlockNumber;

#[derive(Copy, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockNumber(u64);
impl BlockNumber {
    pub fn new(block_number: u64) -> Self {
        BlockNumber(block_number)
    }
}
impl From<PathfinderBlockNumber> for BlockNumber {
    fn from(item: PathfinderBlockNumber) -> Self {
        BlockNumber(item.get())
    }
}
impl Into<PathfinderBlockNumber> for BlockNumber {
    fn into(self) -> PathfinderBlockNumber {
        // `new_or_panic` is acceptable because there is no casting of integers.
        PathfinderBlockNumber::new_or_panic(self.0)
    }
}
// TODO: add `papyrus` block number
