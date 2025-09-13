use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    check::{
        errors::{
            sem_errors::{AnalysisError, ToIdeDiagnostic},
            utils::{add_candidates, get_decl_for_ty},
        },
        recovery::struct_::fuzzy_struct_fields,
    },
    hir_def::interned::identifier::SpanIdent,
    hir_ty::{init_expr_resolver::ResolvedInitExpr, ty::Ty},
    to_proto::ToProto,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitExprError<'db> {
    UnknownStructField {
        ztruct: Ty<'db>,
        field_name: SpanIdent<'db>,
        unknown_field: ResolvedInitExpr<'db>,
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
                    .message(format!(
                        "No field '{}' in STRUCT",
                        field_name.text(db).to_string()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(field_name.get_span(db))
                    .call();

                get_decl_for_ty(db, *ztruct, &mut diag);

                let candidates = fuzzy_struct_fields(db, *ztruct, field_name.as_str(db));
                add_candidates(&candidates, &mut diag);
                diag
            }
        }
    }
}
