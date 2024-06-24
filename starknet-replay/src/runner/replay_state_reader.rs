use blockifier::execution::contract_class::{
    ContractClass as BlockifierContractClass,
    ContractClassV0,
    ContractClassV1,
};
use blockifier::state::state_api::{State, StateReader, StateResult};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use starknet::core::types::ContractClass as StarknetContractClass;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::deprecated_contract_class::ContractClass as DeprecatedContractClass;
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use crate::block_number::BlockNumber;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::rpc::{contract_class, RpcStorage};

pub struct ReplayStateReader<'a> {
    storage: &'a RpcStorage,
    block_number: BlockNumber,
}
impl ReplayStateReader<'_> {
    pub fn new<'a>(storage: &'a RpcStorage, block_number: BlockNumber) -> ReplayStateReader<'a> {
        ReplayStateReader {
            storage,
            block_number,
        }
    }
}
impl StateReader for ReplayStateReader<'_> {
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        let storage_value = self
            .storage
            .starknet_get_storage_at(&self.block_number, &contract_address, &key)
            .unwrap();
        Ok(storage_value)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let block_number_minus_one = BlockNumber::new(self.block_number.get() - 1);
        let nonce = self
            .storage
            .starknet_get_nonce(&block_number_minus_one, &contract_address)
            .unwrap();
        Ok(nonce)
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        let class_hash = self
            .storage
            .starknet_get_class_hash_at(&self.block_number, &contract_address)
            .unwrap();
        Ok(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<BlockifierContractClass> {
        let replay_class_hash = ReplayClassHash {
            block_number: self.block_number,
            class_hash,
        };
        let contract_class = self.storage.starknet_get_class(&replay_class_hash).unwrap();
        match contract_class {
            StarknetContractClass::Sierra(flattened_sierra_cc) => {
                let mut contract_class = serde_json::to_value(flattened_sierra_cc.clone()).unwrap();
                contract_class.as_object_mut().unwrap().remove("abi");
                let sierra_cc: CairoContractClass = serde_json::from_value(contract_class).unwrap();
                let casm_definition =
                    CasmContractClass::from_contract_class(sierra_cc, false, usize::MAX).unwrap();
                let contract_class: ContractClassV1 = casm_definition.try_into().unwrap();
                let contract_class = BlockifierContractClass::V1(contract_class);
                Ok(contract_class)
            }
            StarknetContractClass::Legacy(flattened_casm_cc) => {
                let contract_class = contract_class::decompress(flattened_casm_cc);
                let contract_class = serde_json::to_value(contract_class).unwrap();
                let contract_class: DeprecatedContractClass =
                    serde_json::from_value(contract_class).unwrap();
                Ok(BlockifierContractClass::V0(
                    contract_class.try_into().unwrap(),
                ))
            }
        }
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        todo!()
    }
}
// impl State for ReplayStateReader<'_> {
//     fn set_storage_at(
//         &mut self,
//         contract_address: ContractAddress,
//         key: StorageKey,
//         value: StarkFelt,
//     ) -> StateResult<()> {
//         todo!()
//     }

//     fn increment_nonce(&mut self, contract_address: ContractAddress) ->
// StateResult<()> {         todo!()
//     }

//     fn set_class_hash_at(
//         &mut self,
//         contract_address: ContractAddress,
//         class_hash: ClassHash,
//     ) -> StateResult<()> {
//         todo!()
//     }

//     fn set_contract_class(
//         &mut self,
//         class_hash: ClassHash,
//         contract_class: BlockifierContractClass,
//     ) -> StateResult<()> {
//         todo!()
//     }

//     fn set_compiled_class_hash(
//         &mut self,
//         class_hash: ClassHash,
//         compiled_class_hash: CompiledClassHash,
//     ) -> StateResult<()> {
//         todo!()
//     }

//     fn to_state_diff(&mut self) ->
// blockifier::state::cached_state::CommitmentStateDiff {         todo!()
//     }

//     fn add_visited_pcs(&mut self, class_hash: ClassHash, pcs: &Vec<usize>) {
//         todo!()
//     }
// }
