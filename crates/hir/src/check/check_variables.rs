use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        errors::{analysis_error::{AnalysisError, ToIdeDiagnostic}, duplicates::DuplicateError},
    },
    hir_def::{interned::identifier::Ident, pous::variable::VariableDecl},
    hir_ty::{init_expr_resolver::resolve_init_expr, ty::TyKind},
};

impl<'db> Check<'db> for Vec<VariableDecl<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen: FxHashMap<Ident, VariableDecl<'db>> = FxHashMap::default();
        for variable in self {
            match seen.get(variable.name(db)) {
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
                    seen.insert(*variable.name(db), *variable);
                }
            }

            if let TyKind::Err(err) = variable.spec(db).to_ty(db).kind(db) {
                errors.push(err.to_diagnostic(db));
            }

            if let Some(init) = variable.init(db) {
                check_init_expr(
                    db,
                    variable.spec(db).to_ty(db),
                    *resolve_init_expr(db, variable.spec(db).to_ty(db), *init),
                    errors,
                );
            }
        }
    }
}
