//! This module contains the implementation of [`DebugReplacer`] to add debug
//! information to a [`cairo_lang_sierra::program::Program`] without any. This
//! is because Sierra contracts stored in the Starknet blockchain lack debug
//! data. Without debug information, the [`cairo_lang_sierra::program::Program`]
//! contains only numeric ids to indicate libfuncs and types.

use std::collections::HashSet;
use std::sync::Arc;

use cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId, FunctionId};
use cairo_lang_sierra::program::{self, ConcreteLibfuncLongId, Program, TypeDeclaration};
use cairo_lang_sierra_generator::db::SierraGeneratorTypeLongId;
use cairo_lang_sierra_generator::replace_ids::SierraIdReplacer;
use cairo_lang_utils::extract_matches;

/// Replaces the ids in a Sierra program.
///
/// [`DebugReplacer`] is adapted from
/// [`cairo_lang_sierra_generator::replace_ids::DebugReplacer`]. These changes
/// are required because the [`cairo_lang_sierra_generator::db::SierraGenGroup`]
/// object is not recoverable from Starknet blockchain data.
///
/// This function replaces the dummy ids in
/// [`cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId}`] with a debug
/// string representing the expanded information about the id. For Libfuncs and
/// Types - that would be recursively opening their generic arguments. Function
/// aren't included.
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
///
/// Libfunc `function_call<[id]>` is transformed to `function_call` only because
/// IDs repeat across different contracts and it would have no meaning keeping
/// it.
///
/// [`DebugReplacer`] implements
/// [`cairo_lang_sierra_generator::replace_ids::SierraIdReplacer`] to be able to
/// perform the replacement from id to string.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DebugReplacer<'a> {
    /// The Sierra program to replace ids from.
    program: &'a Program,
}
impl DebugReplacer<'_> {
    /// Get the long debug name for the libfunc with id equivalent to `id`.
    fn lookup_intern_concrete_lib_func(&self, id: &ConcreteLibfuncId) -> ConcreteLibfuncLongId {
        self.program
            .libfunc_declarations
            .iter()
            .find(|f| f.id.id == id.id)
            .expect("ConcreteLibfuncId should be found in libfunc_declarations.")
            .clone()
            .long_id
    }

    /// Get the type declaration for a given `type_id`.
    fn get_type_declaration(&self, type_id: &ConcreteTypeId) -> TypeDeclaration {
        self.program
            .type_declarations
            .iter()
            .find(|f| f.id.id == type_id.id)
            .expect("ConcreteTypeId should be found in type_declarations.")
            .clone()
    }

    /// This function builds the `HashSet` of type dependencies for `type_id`.
    /// The argument `visited_types` is used to keep track of previously
    /// visited dependencies to break cycles and avoid infinite recursion.
    fn type_dependencies(
        &self,
        visited_types: &mut HashSet<ConcreteTypeId>,
        type_id: &ConcreteTypeId,
    ) -> HashSet<ConcreteTypeId> {
        let mut dependencies = HashSet::new();

        if visited_types.contains(type_id) {
            return dependencies;
        }
        visited_types.insert(type_id.clone());

        let concrete_type = self.get_type_declaration(type_id);

        concrete_type.long_id.generic_args.iter().for_each(|t| {
            if let program::GenericArg::Type(concrete_type_id) = t {
                dependencies.insert(concrete_type_id.clone());
                if visited_types.contains(concrete_type_id) {
                    return;
                }
                dependencies.extend(self.type_dependencies(visited_types, concrete_type_id));
            }
        });

        dependencies
    }

    /// Returns true if `type_id` depends on `needle`. False otherwise.
    fn has_in_deps(&self, type_id: &ConcreteTypeId, needle: &ConcreteTypeId) -> bool {
        let mut visited_types = HashSet::new();
        let deps = self.type_dependencies(&mut visited_types, type_id);
        if deps.contains(needle) {
            return true;
        }
        false
    }

    /// Get the long debug name for the type with id equivalent to `id`.
    ///
    /// If `id` is a self-referencing type (i.e. it depends on itself), then the
    /// function returns `None` as an alternative to
    /// [`SierraGeneratorTypeLongId::CycleBreaker`]. It's not possible to
    /// construct a [`SierraGeneratorTypeLongId::CycleBreaker`] object because
    /// it requires having access to the `SalsaDB` of the program.
    fn lookup_intern_concrete_type(
        &self,
        id: &ConcreteTypeId,
    ) -> Option<SierraGeneratorTypeLongId> {
        let concrete_type = self.get_type_declaration(id);
        if self.has_in_deps(id, id) {
            None
        } else {
            Some(SierraGeneratorTypeLongId::Regular(Arc::new(
                concrete_type.long_id,
            )))
        }
    }
}

impl SierraIdReplacer for DebugReplacer<'_> {
    fn replace_libfunc_id(&self, id: &ConcreteLibfuncId) -> ConcreteLibfuncId {
        let mut long_id = self.lookup_intern_concrete_lib_func(id);
        self.replace_generic_args(&mut long_id.generic_args);
        if long_id.generic_id.to_string().starts_with("function_call") {
            long_id.generic_args.clear();
        }
        ConcreteLibfuncId {
            id: id.id,
            debug_name: Some(long_id.to_string().into()),
        }
    }

    fn replace_type_id(&self, id: &ConcreteTypeId) -> ConcreteTypeId {
        match self.lookup_intern_concrete_type(id) {
            // It's not possible to recover the `debug_name` of `Phantom` and `CycleBreaker` because
            // it relies on access to the Salsa db which is available only during
            // contract compilation.
            Some(SierraGeneratorTypeLongId::Regular(long_id)) => {
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
            _ => id.clone(),
        }
    }

    /// Helper for
    /// [`crate::profiler::replace_ids::replace_sierra_ids_in_program`].
    ///
    /// There isn't any replacement of function ids because debug info aren't
    /// recorded in the Starknet blockchain.
    fn replace_function_id(&self, sierra_id: &FunctionId) -> FunctionId {
        sierra_id.clone()
    }
}

/// Returns a sierra `program` with replaced ids.
///
/// This function replaces the dummy ids in
/// [`cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId}`] with a
/// string representing the expanded information about the id. For Libfuncs and
/// Types, that would be recursively opening their generic arguments. For
/// functions no changes are done because of lack of data saved in the
/// blockchain. For example, while the original debug string may be `[6]`, the
/// resulting debug string may be:
///
///  - For libfuncs: `felt252_const<2>` or `unbox<Box<Box<felt252>>>`.
///  - For types: `felt252` or `Box<Box<felt252>>`.
///  - For user functions: `[6]`.
///
/// Similar to
/// [`cairo_lang_sierra_generator::replace_ids::replace_sierra_ids_in_program`]
/// except that it doesn't rely on a
/// [`cairo_lang_sierra_generator::db::SierraGenGroup`] trait object.
#[must_use]
pub fn replace_sierra_ids_in_program(program: &Program) -> Program {
    DebugReplacer { program }.apply(program)
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
    fn libfunc_declaration_eq(a: &[LibfuncDeclaration], b: &[LibfuncDeclaration]) -> bool {
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
    fn type_declaration_eq(a: &[TypeDeclaration], b: &[TypeDeclaration]) -> bool {
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

        assert!(libfunc_declaration_eq(
            &sierra_program_test.libfunc_declarations,
            &sierra_program.libfunc_declarations
        ));

        assert!(type_declaration_eq(
            &sierra_program_test.type_declarations,
            &sierra_program.type_declarations
        ));
    }
}
