use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError},
    hir_ty::{head::init_inference::InitInference, infer::Infer},
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        let mut seen = FxHashMap::default();

        for var in variables {
            match seen.get(&var.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::Variable {
                            var1: *var,
                            var2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(var.get_name_ident(db), *var);
                }
            }

            let var_type = var.spec(db).infer(db);

            if var.variadic(db) && !var_type.normalize(db).can_be_variadic(db) {
                self.errors.push(
                    ResolveError::NonVariadicTypeForVariable {
                        var: *var,
                        typ: var_type,
                    }
                    .to_diagnostic(db),
                );
            }

            if let Some(init_expr) = var.init(db) {
                self.init_expr_result.resolve_init_expr(
                    db,
                    init_expr,
                    &mut self.body_infer_result,
                    var_type,
                );
            }
        }
    }
}
