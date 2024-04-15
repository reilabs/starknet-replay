use cairo_lang_starknet_classes::contract_class::ContractClass;
use cairo_lang_starknet_classes::felt252_serde::sierra_from_felt252s;
use std::fs::read_to_string;

pub fn read_json_file(filename: &str) -> serde_json::Value {
    let json_str = read_to_string(filename).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

fn main() {
    let filename = r#"C:\Users\Giuseppe\source\repos\cairo\crates\cairo-lang-starknet\test_data\minimal_contract.contract_class.json"#;
    let contract_class: ContractClass = serde_json::from_value(read_json_file(filename)).unwrap();

    let (_, _, sierra_program) = sierra_from_felt252s(&contract_class.sierra_program).unwrap();

    sierra_program
        .libfunc_declarations
        .iter()
        .for_each(|f| println!("{:#?}", f.long_id.generic_id.to_string()));
}
