use auto_lsp::default::db::BaseDatabase;

use crate::hir_ty::ty::{CallableType, Type};

impl<'db> Type<'db> {
    pub fn normalize(&self, db: &'db dyn BaseDatabase) -> Type<'db> {
        match self {
            Type::DataType(dt) => Type::new_spec(db, dt.spec(db)),
            Type::Variable(var) => Type::new_spec(db, var.spec(db)).normalize(db),
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