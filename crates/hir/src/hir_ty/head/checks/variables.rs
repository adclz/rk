use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError,
        e2_resolve::ResolveError, e3_type::{InferLiteralError, TypeError},
    },
    hir_def::expressions::{
        expression::{Elementary, ExprKind, InitExpr, InitExprKind, PrimaryExpr},
        spec::SpecKind,
    },
    hir_ty::{head::init_inference::InitInference, infer::Infer, ty::Type},
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

                // Check string literal length for sized string specs
                self.check_sized_string_init(db, var.spec(db).kind(db), init_expr);
            }
        }
    }

    fn check_sized_string_init(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec_kind: &SpecKind<'db>,
        init: InitExpr<'db>,
    ) {
        let (max_len_expr, is_wstring) = match spec_kind {
            SpecKind::SizedString(length) => (length, false),
            SpecKind::SizedWString(length) => (length, true),
            _ => return,
        };

        let max_len = match max_len_expr.as_range(db) {
            Some(len) => len,
            None => return,
        };

        // Only check simple constant expression initializers
        let InitExprKind::ConstantExpr(expr) = init.kind(db) else {
            return;
        };

        let ExprKind::PrimaryExpr(PrimaryExpr::Literal(elem)) = expr.expr(db) else {
            return;
        };

        let actual_len = match elem {
            Elementary::String(s) if !is_wstring => s.as_single_string(db).ok().map(|v| v.len()),
            Elementary::WString(s) if is_wstring => s.as_double_string(db).ok().map(|v| v.len()),
            _ => None,
        };

        if let Some(actual_len) = actual_len {
            if actual_len as u64 > max_len {
                let err = if is_wstring {
                    InferLiteralError::Invalid_WSTRING_Length {
                        max: max_len,
                        got: actual_len,
                    }
                } else {
                    InferLiteralError::Invalid_STRING_Length {
                        max: max_len,
                        got: actual_len,
                    }
                };
                let target = if is_wstring {
                    Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::WString)
                } else {
                    Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::String)
                };
                self.errors.push(
                    TypeError::InferLiteralError {
                        expr,
                        source: None,
                        target,
                        err,
                    }
                    .to_diagnostic(db),
                );
            }
        }
    }
}
