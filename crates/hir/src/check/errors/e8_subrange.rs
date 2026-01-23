use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::expressions::spec::Spec,
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SubRangeError<'db> {
    // Subrange
    InvalidSubrangeType { spec: Spec<'db>, typ: Type<'db> },
}

impl<'db> From<SubRangeError<'db>> for AnalysisError<'db> {
    fn from(err: SubRangeError<'db>) -> Self {
        AnalysisError::SubRange(err)
    }
}

impl<'db> ErrorCode for SubRangeError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidSubrangeType { .. } => "E0801",
        }
    }

    fn description(&self) -> &'static str {
        "invalid subrange type"
    }
}

impl<'db> ToIdeDiagnostic<'db> for SubRangeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            SubRangeError::InvalidSubrangeType { spec, typ } => {
                let mut diag = diag()
                    .message(format!("Invalid subrange type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(spec.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for SUBRANGE".to_string());

                diag
            }
        }
    }
}
