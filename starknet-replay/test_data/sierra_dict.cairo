use core::dict::Felt252Dict;
#[starknet::interface]
trait HelloStarknetTrait<TContractState> {
    fn increase_balance(ref self: TContractState);
}
#[starknet::contract]
mod hello_starknet {
    #[storage]
    struct Storage {
    }
    #[abi(embed_v0)]
    impl HelloStarknetImpl of super::HelloStarknetTrait<ContractState> {
        fn increase_balance(ref self: ContractState) {
            let mut i: u8 = 0;
            loop {
                if i >= 100 {
                    break;
                }
                let mut dict: Felt252Dict<u8> = Default::default();
                dict.insert(i.into(), i);
                i = i + 1;
            };
        }
    }
}
