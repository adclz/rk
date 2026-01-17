use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::{
        check_semantic_index::Check,
        errors::{
            analysis_error::ToIdeDiagnostic, body_inference::BodyInferenceError,
            duplicates::DuplicateError,
        },
    },
    hir_def::{interned::identifier::Ident, pous::variable::VariableDecl},
    hir_ty::{init_inference::infer_variable, ty::Type},
};

impl<'db> Check<'db> for [VariableDecl<'db>] {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
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
                errors.push(
                    BodyInferenceError::NoSpecItemInScope {
                        spec: variable.spec(db),
                        scope: variable.scope_id(db),
                    }
                    .to_diagnostic(db),
                );
            }

            // Check initializer expression

            if let Some(init_expr) = variable.init(db) {
                let infer = infer_variable(db, *variable);
                for error in infer.errors.iter() {
                    errors.push(error.clone());
                }

                for error in infer.body_infer_result.errors.iter() {
                    errors.push(error.clone());
                }
            }
        }
    }
}
