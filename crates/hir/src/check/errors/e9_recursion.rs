use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo, check::errors::ToIdeDiagnostic, hir_def::pous::pou::Pou,
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

impl ErrorCode for RecursionError<'_> {
    fn code(&self) -> &'static str {
        match self {
            Self::DirectRecursion { .. } => "E0901",
            Self::MutualRecursion { .. } => "E0902",
        }
    }

    fn description(&self) -> &'static str {
        "recursion detected"
    }
}

impl<'db> ToIdeDiagnostic<'db> for RecursionError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase, file: auto_lsp::default::db::file::File) -> IdeDiagnostic {
        match self {
            RecursionError::DirectRecursion { pou, callsite } => {
                let mut diag = diag()
                    .message(format!(
                        "type '{}' is recursive (contains itself)",
                        pou.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &pou.get_name_span(db)).unwrap_or_default())
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
                    .desc(self)
                    .range(crate::denormalize(db, file, &pou.get_name_span(db)).unwrap_or_default())
                    .call();

                for cs in callsite {
                    diag.with_related(Related::new(
                        "recurses at this location".to_string(),
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
                        "cycle goes\n{stack}... and back to {}",
                        pou.get_name_ident(db).text(db)
                    ));
                }

                diag
            }
        }
    }
}
