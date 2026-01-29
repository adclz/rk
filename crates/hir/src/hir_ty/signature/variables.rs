use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
    },
    hir_def::expressions::spec::SpecKind,
    hir_ty::{signature::Signature, ty::Type},
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        let mut seen = FxHashMap::default();
        for var in variables.iter() {
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

            let var_type = Type::new_spec(db, var.spec(db));
            if var_type.is_never() {
                if let SpecKind::Target(target) = var.spec(db).kind(db) {
                    self.errors.push(
                        ResolveError::NoNamespaceItemFound {
                            path: target.clone(),
                        }
                        .to_diagnostic(db),
                    );
                }
                self.type_of_specs.insert(var.spec(db), Type::Never);
                continue;
            }
            self.type_of_specs.insert(var.spec(db), var_type);

            if let Some(init_expr) = var.init(db) {
                self.init_expr_result
                    .resolve_init_expr(db, init_expr, &mut self.body_infer_result, var_type);
            }
        }
    }
}
