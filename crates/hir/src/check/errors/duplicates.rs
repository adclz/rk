use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{diag, IdeDiagnostic, Related};

use crate::{check::errors::sem_errors::ToIdeDiagnostic, hir_def::pous::{pou::PouDecl, variable::VariableDecl}, to_proto::ToProto};



#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Variable {
        var1: VariableDecl<'db>,
        var2: VariableDecl<'db>,
    },
    Pou {
        pou1: PouDecl<'db>,
        pou2: PouDecl<'db>,
    },
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "variable '{}' is already defined",
                        var1.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("variable '{}' is defined here", var2.name(db).text(db)),
                    var2.scope_id(db).file(db),
                    var2.get_span(db),
                ));

                diag
            }
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!(
                        "POU '{}' is already defined",
                        pou1.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("POU '{}' is defined here", pou2.name(db).text(db)),
                    pou2.scope_id(db).file(db),
                    pou2.get_span(db),
                ));

                diag
            }
        }
    }
}
