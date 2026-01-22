use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::pous::pou::Pou,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RecursionError<'db> {
    DirectRecursion {
        pou: Pou<'db>,
        callsite: Option<CallSite<'db>>,
    },
    MutualRecursion {
        pou: Pou<'db>,
        pous: Vec<Pou<'db>>,
        callsite: Vec<CallSite<'db>>,
    },
}

impl<'db> From<RecursionError<'db>> for AnalysisError<'db> {
    fn from(err: RecursionError<'db>) -> Self {
        AnalysisError::RecursionError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for RecursionError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            RecursionError::DirectRecursion { pou, callsite } => {
                let mut diag = diag()
                    .message(format!(
                        "type '{}' is recursive (contains itself)",
                        pou.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou.get_name_span(db))
                    .call();

                if let Some(cs) = callsite {
                    diag.with_related(Related::new(
                        format!(
                            "'{}' references itself here",
                            pou.get_name_ident(db).text(db)
                        ),
                        cs.get_scope_id(db).file(db),
                        cs.get_span(db),
                    ));
                }

                diag
            }
            RecursionError::MutualRecursion {
                pou,
                pous,
                callsite,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "type '{}' is recursive",
                        pou.get_name_ident(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou.get_name_span(db))
                    .call();

                for cs in callsite {
                    diag.with_related(Related::new(
                        "recurse at this location".to_string(),
                        cs.get_scope_id(db).file(db),
                        cs.get_span(db),
                    ));
                }

                if !pous.is_empty() {
                    let stack = pous
                        .iter()
                        .map(|p| format!("-> {}\n", p.get_name_ident(db).text(db)))
                        .collect::<Vec<_>>()
                        .join("");

                    diag.with_note(format!(
                        "cycles goes\n{stack}... and back to {}",
                        pou.get_name_ident(db).text(db)
                    ));
                }

                diag
            }
        }
    }
}
