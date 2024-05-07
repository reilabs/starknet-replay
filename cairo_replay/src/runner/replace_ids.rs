use std::sync::Arc;

use cairo_lang_sierra::program::{self};
use cairo_lang_sierra_generator::db::SierraGeneratorTypeLongId;
use cairo_lang_sierra_generator::replace_ids::SierraIdReplacer;
use cairo_lang_utils::extract_matches;

/// Replaces `cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId,
/// FunctionId}` with a dummy ids whose debug string is the string representing
/// the expanded information about the id. For Libfuncs and Types - that would
/// be recursively opening their generic arguments, for functions - that would
/// be getting their original name. For example, while the original debug string
/// may be `[6]`, the resulting debug string may be:
///  - For libfuncs: `felt252_const<2>` or `unbox<Box<Box<felt252>>>`.
///  - For types: `felt252` or `Box<Box<felt252>>`.
///  - For user functions: `test::foo`.
#[derive(Debug, Clone, Eq, PartialEq)]
struct DebugReplacer {
    program: cairo_lang_sierra::program::Program,
}
impl DebugReplacer {
    fn lookup_intern_concrete_lib_func(
        &self,
        id: cairo_lang_sierra::ids::ConcreteLibfuncId,
    ) -> cairo_lang_sierra::program::ConcreteLibfuncLongId {
        self.program
            .libfunc_declarations
            .iter()
            .find(|f| f.id.id == id.id)
            .unwrap()
            .clone()
            .long_id
    }

    fn lookup_intern_concrete_type(
        &self,
        id: cairo_lang_sierra::ids::ConcreteTypeId,
    ) -> SierraGeneratorTypeLongId {
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
    fn replace_libfunc_id(
        &self,
        id: &cairo_lang_sierra::ids::ConcreteLibfuncId,
    ) -> cairo_lang_sierra::ids::ConcreteLibfuncId {
        let mut long_id = self.lookup_intern_concrete_lib_func(id.clone());
        self.replace_generic_args(&mut long_id.generic_args);
        cairo_lang_sierra::ids::ConcreteLibfuncId {
            id: id.id,
            debug_name: Some(long_id.to_string().into()),
        }
    }

    fn replace_type_id(
        &self,
        id: &cairo_lang_sierra::ids::ConcreteTypeId,
    ) -> cairo_lang_sierra::ids::ConcreteTypeId {
        match self.lookup_intern_concrete_type(id.clone()) {
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
                cairo_lang_sierra::ids::ConcreteTypeId {
                    id: id.id,
                    debug_name: Some(long_id.to_string().into()),
                }
            }
        }
    }

    /// Helper for [replace_sierra_ids] and [replace_sierra_ids_in_program]
    /// replacing function ids.
    fn replace_function_id(
        &self,
        sierra_id: &cairo_lang_sierra::ids::FunctionId,
    ) -> cairo_lang_sierra::ids::FunctionId {
        sierra_id.clone()
    }
}

/// Replaces `cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId,
/// FunctionId}` with a dummy ids whose debug string is the string representing
/// the expanded information about the id. For Libfuncs and Types - that would
/// be recursively opening their generic arguments, for functions - that would
/// be getting their original name. For example, while the original debug string
/// may be `[6]`, the resulting debug string may be:
///  - For libfuncs: `felt252_const<2>` or `unbox<Box<Box<felt252>>>`.
///  - For types: `felt252` or `Box<Box<felt252>>`.
///  - For user functions: `test::foo`.
///
/// Similar to [replace_sierra_ids] except that it acts on
/// [cairo_lang_sierra::program::Program].
pub fn replace_sierra_ids_in_program(
    program: &cairo_lang_sierra::program::Program,
) -> cairo_lang_sierra::program::Program {
    DebugReplacer {
        program: program.clone(),
    }
    .apply(program)
}
