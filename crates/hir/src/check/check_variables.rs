use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        errors::{analysis_error::ToIdeDiagnostic, body_inference::{BodyInferenceError, TypeError}, duplicates::DuplicateError},
    }, hir_def::{interned::identifier::Ident, pous::variable::VariableDecl}, hir_ty::ty::Type
};

impl<'db> Check<'db> for [VariableDecl<'db>] {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen: FxHashMap<Ident, VariableDecl<'db>> = FxHashMap::default();
        for variable in self {
            match seen.get(&variable.get_name_ident(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::Variable {
                            var1: *variable,
                            var2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(variable.get_name_ident(db), *variable);
                }
            }

            let var_typ = Type::new_spec(db, variable.spec(db));
            if var_typ.is_never() {
                eprintln!("checking variable with never type: {:?}", variable.name(db));
                errors.push(
                    BodyInferenceError::NoSpecItemInScope { spec: variable.spec(db), scope: variable.scope_id(db) }
                    .to_diagnostic(db)
                );
            }

            // Check initializer expression

            if let Some(init_expr) = variable.init(db) {
                check_init_expr(
                    db,
                    Type::new_spec(db, variable.spec(db)),
                    *init_expr,
                    errors,
                );
            }
        }
    }
}
