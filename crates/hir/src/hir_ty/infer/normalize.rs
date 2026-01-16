use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;

use crate::hir_ty::ty::{CallableType, Type};

/*
    Normalizes a type into its identity data type.

    This operation:
    - Resolves Variable references to their declared data types
    - Resolves DataType aliases to their underlying specifications
    - Extracts and normalizes the return type of callable types
    - Recursively removes all transparent layers until a concrete data type
      (or Void / Never) is reached

    This is an eager, lossy operation: information about how a value was
    reached (variable access, path steps, call origin, etc.) is discarded.
*/

impl<'db> Type<'db> {
    pub fn normalize(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self {
            Type::DataType(dt) => Type::new_spec(db, dt.spec(db)),
            Type::Variable((var, multibits)) => Type::new_spec(db, var.spec(db)).normalize(db),
            Type::CallableType(typ) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => Type::new_spec(db, *ret_ty).normalize(db),
                    _ => Type::Void,
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => Type::new_spec(db, *ret_ty).normalize(db),
                    _ => Type::Void,
                },
                _ => *self,
            },
            _ => *self,
        }
    }
}
