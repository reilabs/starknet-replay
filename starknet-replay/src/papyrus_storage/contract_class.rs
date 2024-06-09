use cairo_lang_utils::bigint::BigUintAsHex;
use num_bigint::BigUint;
use starknet_api::hash::StarkFelt;
use starknet_api::state::ContractClass as PapyrusContractClass;

use crate::common::ContractClass;

fn starkfelt_to_biguint(item: StarkFelt) -> BigUintAsHex {
    let big_int = BigUint::from_bytes_be(item.bytes());
    let out = BigUintAsHex { value: big_int };
    println!("In {:?} | Out {:?}", item, out);
    out
}

fn biguint_to_starkfelt(item: BigUintAsHex) -> StarkFelt {
    let bytes: [u8; 32] = item.value.to_bytes_be().try_into().unwrap();
    let out = StarkFelt::new(bytes).unwrap();
    println!("In {:?} | Out {:?}", item, out);
    out
}

impl From<PapyrusContractClass> for ContractClass {
    fn from(item: PapyrusContractClass) -> Self {
        ContractClass {
            compressed_sierra_program: item
                .sierra_program
                .iter()
                .map(|&x| starkfelt_to_biguint(x))
                .collect(),
            contract_class_version: "".to_string(),
            entry_points_by_type: item.entry_points_by_type,
            abi: item.abi,
        }
    }
}
impl From<ContractClass> for PapyrusContractClass {
    fn from(val: ContractClass) -> Self {
        PapyrusContractClass {
            sierra_program: val
                .compressed_sierra_program
                .iter()
                .map(|x| biguint_to_starkfelt(x.clone()))
                .collect(),
            entry_points_by_type: val.entry_points_by_type,
            abi: val.abi,
        }
    }
}
