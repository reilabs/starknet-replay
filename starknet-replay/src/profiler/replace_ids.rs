//! This module contains the implementation of `DebugReplacer` to add debug
//! information to a `Program` without any. This is because Sierra contracts
//! stored in the Starknet blockchain lack debug data. Without debug
//! information, the `Program` contains only numeric ids to indicate libfuncs
//! and types.

use std::sync::Arc;

use cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId, FunctionId};
use cairo_lang_sierra::program::{self, ConcreteLibfuncLongId, Program};
use cairo_lang_sierra_generator::db::SierraGeneratorTypeLongId;
use cairo_lang_sierra_generator::replace_ids::SierraIdReplacer;
use cairo_lang_utils::extract_matches;

/// Replaces the ids in a Sierra program.
///
/// `DebugReplacer` is adapted from `DebugReplacer` contained in the crate
/// `cairo-lang-sierra-generator`. These changes are required because the
/// `SierraGenGroup` object is not recoverable from Starknet blockchain data.
///
/// This function replaces `cairo_lang_sierra::ids::{ConcreteLibfuncId,
/// ConcreteTypeId}` with a dummy ids whose debug string is the string
/// representing the expanded information about the id. For Libfuncs and Types -
/// that would be recursively opening their generic arguments. Function aren't
/// included.
///
/// This is needed because the Sierra Bytecode stored in the database
/// requires id replacement for ease of readability.
///
/// For example, while the original debug string may be `[6]`, the
/// resulting debug string may be:
///  - For libfuncs: `felt252_const<2>` or `unbox<Box<Box<felt252>>>`.
///  - For types: `felt252` or `Box<Box<felt252>>`.
///  - For user functions: `[6]`.
///
/// User functions are kept in numeric id form because the names aren't
/// recoverable after the contract is compiled and deployed in the blockchain.
/// `DebugReplacer` implements `SierraIdReplacer` to be able to perform the
/// replacement from id to string.
#[derive(Debug, Clone, Eq, PartialEq)]
struct DebugReplacer {
    /// The Sierra program to replace ids from.
    program: Program,
}
impl DebugReplacer {
    /// Get the long debug name for the libfunc with id equivalent to `id`.
    fn lookup_intern_concrete_lib_func(&self, id: &ConcreteLibfuncId) -> ConcreteLibfuncLongId {
        self.program
            .libfunc_declarations
            .iter()
            .find(|f| f.id.id == id.id)
            .unwrap()
            .clone()
            .long_id
    }

    /// Get the long debug name for the type with id equivalent to `id`.
    fn lookup_intern_concrete_type(&self, id: &ConcreteTypeId) -> SierraGeneratorTypeLongId {
        let concrete_type = self
            .program
            .type_declarations
            .iter()
            .find(|f| f.id.id == id.id)
            .unwrap()
            .clone();
        SierraGeneratorTypeLongId::Regular(Arc::new(concrete_type.long_id))
    }
}
impl SierraIdReplacer for DebugReplacer {
    fn replace_libfunc_id(&self, id: &ConcreteLibfuncId) -> ConcreteLibfuncId {
        let mut long_id = self.lookup_intern_concrete_lib_func(id);
        self.replace_generic_args(&mut long_id.generic_args);
        if long_id.generic_id.to_string().starts_with("function_call") {
            //long_id.generic_id = "function_call".into();
            long_id.generic_args.clear();
        }
        ConcreteLibfuncId {
            id: id.id,
            debug_name: Some(long_id.to_string().into()),
        }
    }

    fn replace_type_id(&self, id: &ConcreteTypeId) -> ConcreteTypeId {
        match self.lookup_intern_concrete_type(id) {
            SierraGeneratorTypeLongId::CycleBreaker(ty) => todo!("{:?}", ty),
            SierraGeneratorTypeLongId::Regular(long_id) => {
                let mut long_id = long_id.as_ref().clone();
                self.replace_generic_args(&mut long_id.generic_args);
                if long_id.generic_id == "Enum".into() || long_id.generic_id == "Struct".into() {
                    long_id.generic_id =
                        extract_matches!(&long_id.generic_args[0], program::GenericArg::UserType)
                            .to_string()
                            .into();
                    if long_id.generic_id == "Tuple".into() {
                        long_id.generic_args = long_id.generic_args.into_iter().skip(1).collect();
                        if long_id.generic_args.is_empty() {
                            long_id.generic_id = "Unit".into();
                        }
                    } else {
                        long_id.generic_args.clear();
                    }
                }
                ConcreteTypeId {
                    id: id.id,
                    debug_name: Some(long_id.to_string().into()),
                }
            }
        }
    }

    /// Helper for [`replace_sierra_ids`] and [`replace_sierra_ids_in_program`]
    /// replacing function ids.
    fn replace_function_id(&self, sierra_id: &FunctionId) -> FunctionId {
        sierra_id.clone()
    }
}

/// Returns a sierra `program` with replaced ids.
///
/// This function replaces `cairo_lang_sierra::ids::{ConcreteLibfuncId,
/// ConcreteTypeId}` with a dummy ids whose debug string is the string
/// representing the expanded information about the id. For Libfuncs and Types,
/// that would be recursively opening their generic arguments. For functions no
/// changes are done because of lack of data saved in the blockchain. For
/// example, while the original debug string may be `[6]`, the resulting debug
/// string may be:
///
///  - For libfuncs: `felt252_const<2>` or `unbox<Box<Box<felt252>>>`.
///  - For types: `felt252` or `Box<Box<felt252>>`.
///  - For user functions: `[6]`.
///
/// Similar to [`replace_sierra_ids`] except that it acts on
/// [`cairo_lang_sierra::program::Program`].
#[must_use]
pub fn replace_sierra_ids_in_program(program: &Program) -> Program {
    DebugReplacer {
        program: program.clone(),
    }
    .apply(program)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io};

    use cairo_lang_sierra::program::{LibfuncDeclaration, Program, TypeDeclaration};
    use itertools::Itertools;

    use super::*;

    fn read_test_file(filename: &str) -> io::Result<String> {
        let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let sierra_program_json_file = [out_dir.as_str(), filename].iter().join("");
        let sierra_program_json_file = sierra_program_json_file.as_str();
        fs::read_to_string(sierra_program_json_file)
    }

    // This is because the built-in equality doesn't check for matching `debug_name`
    // string.
    fn compare_libfunc_declaration(a: &[LibfuncDeclaration], b: &[LibfuncDeclaration]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        for (a, b) in a
            .iter()
            .sorted_by(|a, b| Ord::cmp(&a.id.id, &b.id.id))
            .zip(b.iter().sorted_by(|a, b| Ord::cmp(&a.id.id, &b.id.id)))
        {
            if a.id.id != b.id.id {
                println!("Different ids. Expected {:?} | Actual {:?}", a.id, b.id);
                return false;
            }
            if a.id.debug_name != b.id.debug_name {
                println!(
                    "Different debug_name. Expected {:?} | actual {:?}",
                    a.id, b.id
                );
                return false;
            }
        }
        true
    }

    // This is because the built-in equality doesn't check for matching `debug_name`
    // string.
    fn compare_type_declaration(a: &[TypeDeclaration], b: &[TypeDeclaration]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        for (a, b) in a
            .iter()
            .sorted_by(|a, b| Ord::cmp(&a.id.id, &b.id.id))
            .zip(b.iter().sorted_by(|a, b| Ord::cmp(&a.id.id, &b.id.id)))
        {
            if a.id.id != b.id.id {
                println!("Different ids {:?} {:?}", a.id, b.id);
                return false;
            }
            if a.id.debug_name != b.id.debug_name {
                println!("Different ids {:?} {:?}", a.id, b.id);
                return false;
            }
        }
        true
    }

    #[test]
    fn test_replace_id() {
        let sierra_program_file = "/test_data/sierra_program.json";
        let sierra_program_json = read_test_file(sierra_program_file)
            .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_file}"));
        let sierra_program_json: serde_json::Value = serde_json::from_str(&sierra_program_json)
            .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_file} to json"));
        let sierra_program: Program = serde_json::from_value::<Program>(sierra_program_json)
            .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_file} to Program"));
        let sierra_program = replace_sierra_ids_in_program(&sierra_program);

        let sierra_program_test_file = "/test_data/sierra_program_replaced_id.json";
        let sierra_program_test_json = read_test_file(sierra_program_test_file)
            .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_test_file}"));
        let sierra_program_test_json: serde_json::Value =
            serde_json::from_str(&sierra_program_test_json)
                .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_test_file} to json"));
        let sierra_program_test: Program =
            serde_json::from_value::<Program>(sierra_program_test_json).unwrap_or_else(|_| {
                panic!("Unable to parse {sierra_program_test_file} to Program")
            });

        assert!(compare_libfunc_declaration(
            &sierra_program_test.libfunc_declarations,
            &sierra_program.libfunc_declarations
        ));

        assert!(compare_type_declaration(
            &sierra_program_test.type_declarations,
            &sierra_program.type_declarations
        ));
    }
}
