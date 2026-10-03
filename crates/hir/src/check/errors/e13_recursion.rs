// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::pous::pou::Pou;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

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

impl<'db> ErrorCode for RecursionError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::DirectRecursion { .. } => "E1301",
            Self::MutualRecursion { .. } => "E1302",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for RecursionError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            RecursionError::DirectRecursion { pou, callsite } => {
                let mut diag = diag()
                    .message(format!(
                        "type '{}' contains itself",
                        pou.get_name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &pou.get_name_span(db)).unwrap_or_default())
                    .call();

                if let Some(cs) = callsite {
                    diag.with_related(Related::new(
                        format!(
                            "'{}' references itself here",
                            pou.get_name_with_case(db).text(db)
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
                        pou.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &pou.get_name_span(db)).unwrap_or_default())
                    .call();

                for cs in callsite {
                    diag.with_related(Related::new(
                        "the cycle passes here".to_string(),
                        cs.get_scope_id(db).file(db),
                        cs.get_span(db),
                    ));
                }

                if !pous.is_empty() {
                    let stack = pous
                        .iter()
                        .map(|p| format!("-> {}\n", p.get_name_with_case(db).text(db)))
                        .collect::<Vec<_>>()
                        .join("");

                    diag.with_note(format!(
                        "cycle goes\n{stack}... and back to {}",
                        pou.get_name_with_case(db).text(db)
                    ));
                }

                diag
            }
        }
    }
}
