use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        check_ty::check_ty,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError},
    },
    hir_def::pous::variable::VariableDecl,
    hir_ty::{init_expr_resolver::resolve_init_expr, ty::ty_for_variable},
};

impl<'db> Check<'db> for Vec<VariableDecl<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let mut seen = FxHashMap::default();
        for variable in self {
            match seen.get(variable.name(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::Variable {
                            var1: ty_for_variable(db, *variable),
                            var2: ty_for_variable(db, *prev),
                        }
                        .into(),
                    );
                }
                None => {
                    seen.insert(variable.name(db), *variable);
                }
            }

            let var = ty_for_variable(db, *variable);
            check_ty(db, var, errors);
            if let Some(init) = variable.init(db) {
                check_init_expr(db, var, *resolve_init_expr(db, ty_for_variable(db, *variable), *init), errors);
            }
        }
    }
}
