use starknet_api::block::BlockNumber as PapyrusBlockNumber;

use crate::common::BlockNumber;

impl From<PapyrusBlockNumber> for BlockNumber {
    fn from(item: PapyrusBlockNumber) -> Self {
        BlockNumber::new(item.0)
    }
}
impl From<BlockNumber> for PapyrusBlockNumber {
    fn from(val: BlockNumber) -> Self {
        // `new_or_panic` is acceptable because there is no casting of integers.
        PapyrusBlockNumber(val.get())
    }
}
