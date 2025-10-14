use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        check_ty::check_ty,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError},
    },
    hir_def::{interned::identifier::Ident, pous::variable::VariableDecl},
    hir_ty::init_expr_resolver::resolve_init_expr,
};

impl<'db> Check<'db> for Vec<VariableDecl<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let mut seen: FxHashMap<Ident, VariableDecl<'db>> = FxHashMap::default();
        for variable in self {
            match seen.get(variable.name(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::Variable {
                            var1: variable.spec(db).spec_to_ty(db),
                            var2: prev.spec(db).spec_to_ty(db),
                        }
                        .into(),
                    );
                }
                None => {
                    seen.insert(*variable.name(db), *variable);
                }
            }

            let var = variable.spec(db).spec_to_ty(db);
            check_ty(db, var, errors);
            if let Some(init) = variable.init(db) {
                check_init_expr(db, var, *resolve_init_expr(db, var, *init), errors);
            }
        }
    }
}
