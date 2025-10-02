use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::{
        errors::{
            analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
            coerce::ExprMismatch,
            utils::{get_candidates, get_decl_for_ty},
        },
        recovery::struct_::fuzzy_struct_fields,
    },
    hir_def::interned::identifier::SpanIdent,
    hir_ty::{expr_resolver::ResolvedExpr, init_expr_resolver::ResolvedInitExpr, ty::Ty},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitExprError<'db> {
    UnknownStructField {
        ztruct: Ty<'db>,
        field_name: SpanIdent<'db>,
        unknown_field: ResolvedInitExpr<'db>,
    },
    ArrayTooManyElements {
        array: Ty<'db>,
        provided_count: u64,
        max_capacity: u64,
        init_expr: ResolvedInitExpr<'db>,
    },
    InitExprTypeExprMismatch {
        err: ExprMismatch<'db>,
        init_expr: ResolvedExpr<'db>,
    },
}

impl<'db> From<InitExprError<'db>> for AnalysisError<'db> {
    fn from(err: InitExprError<'db>) -> Self {
        AnalysisError::InitExprError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for InitExprError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            InitExprError::UnknownStructField {
                ztruct,
                field_name,
                unknown_field,
            } => {
                let mut diag = diag()
                    .message(format!("no field '{}' in STRUCT", field_name.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(field_name.get_span(db))
                    .call();

                get_decl_for_ty(db, *ztruct, &mut diag);

                let candidates = fuzzy_struct_fields(db, *ztruct, field_name.as_str(db));
                diag.with_note(get_candidates(&candidates));
                diag
            }
            InitExprError::ArrayTooManyElements {
                array,
                provided_count,
                max_capacity,
                init_expr,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "too many array elements: provided {provided_count}, but array capacity is {max_capacity}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(init_expr.get_span(db))
                    .call();

                get_decl_for_ty(db, *array, &mut diag);
                diag
            }
            InitExprError::InitExprTypeExprMismatch { err, init_expr } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid value initializer: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(init_expr.get_span(db))
                    .call();

                err.note(db, &mut diag);
                err.related(db, &mut diag);
                diag
            }
        }
    }
}
