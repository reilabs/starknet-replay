use pathfinder_common::BlockNumber as PathfinderBlockNumber;

#[derive(Copy, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockNumber(u64);
impl BlockNumber {
    #[must_use]
    pub fn new(block_number: u64) -> Self {
        BlockNumber(block_number)
    }

    #[must_use]
    pub fn get(&self) -> u64 {
        self.0
    }
}
impl From<PathfinderBlockNumber> for BlockNumber {
    fn from(item: PathfinderBlockNumber) -> Self {
        BlockNumber(item.get())
    }
}
impl From<BlockNumber> for PathfinderBlockNumber {
    fn from(val: BlockNumber) -> Self {
        // `new_or_panic` is acceptable because there is no casting of integers.
        PathfinderBlockNumber::new_or_panic(val.0)
    }
}
// TODO: add `papyrus` block number
