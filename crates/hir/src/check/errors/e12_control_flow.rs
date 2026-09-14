use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::statement::Stmt;
use crate::hir_def::pous::variable::VariableDecl;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ControlFlowError<'db> {
    ExitOutsideLoop {
        stmt: Stmt<'db>,
    },
    ContinueOutsideLoop {
        stmt: Stmt<'db>,
    },
    /// A FOR control variable that is not a bare identifier. IEC 61131-3's
    /// grammar says `control_variable ::= identifier`, so `r.i`, `a[k]` and
    /// `p^` cannot be counters. (A bare name
    /// that RESOLVES to an FB/PROGRAM member is fine; the restriction is on
    /// the syntax, not on where the variable lives.)
    ForControlNotAVariable {
        access: CallSite<'db>,
    },
    /// A FOR step whose value the compiler cannot fix at compile time, or a
    /// zero one. The sign decides the loop's exit comparison, so both are
    /// loops whose direction is unknowable.
    ForStepInvalid {
        step: CallSite<'db>,
        zero: bool,
        /// The variable the step names, when it names exactly one - the
        /// declaration to point at with the fix (qualify it CONSTANT).
        decl: Option<VariableDecl<'db>>,
    },
    /// A case label must evaluate to a constant value at compile time.
    CaseLabelNotConstant {
        label: CallSite<'db>,
        /// Whether the label is a RANGE BOUND. A single label may be a
        /// string or an enum variant; a bound may not, because a range is an
        /// ordering and those do not order — so the two need different
        /// wording for what is otherwise the same rule.
        as_range_bound: bool,
    },
}

impl<'db> ErrorCode for ControlFlowError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::ExitOutsideLoop { .. } => "E1201",
            Self::ContinueOutsideLoop { .. } => "E1202",
            Self::ForControlNotAVariable { .. } => "E1203",
            Self::ForStepInvalid { .. } => "E1204",
            Self::CaseLabelNotConstant { .. } => "E1205",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::ExitOutsideLoop { .. } => "control flow violation",
            Self::ContinueOutsideLoop { .. } => "control flow violation",
            Self::ForControlNotAVariable { .. } => "control flow violation",
            Self::ForStepInvalid { .. } => "control flow violation",
            Self::CaseLabelNotConstant { .. } => "control flow violation",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ControlFlowError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::ExitOutsideLoop { stmt } => diag()
                .message("'EXIT' can only be used inside loops".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ContinueOutsideLoop { stmt } => diag()
                .message("'CONTINUE' can only be used inside loops".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ForControlNotAVariable { access } => {
                let mut diag = diag()
                    .message("a FOR control variable must be a plain variable".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "count in a plain variable and assign it where it is needed inside the loop"
                        .to_string(),
                );
                diag
            }
            Self::ForStepInvalid { step, zero, decl } => {
                let mut diag = diag()
                    .message(
                        if *zero {
                            "a FOR step of zero never advances the loop"
                        } else {
                            "a FOR step must evaluate to a constant at compile time"
                        }
                        .to_string(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &step.get_span(db)).unwrap_or_default())
                    .call();
                if let Some(var) = decl {
                    let site = var.as_call_site(db);
                    diag.with_related(Related::new(
                        format!(
                            "declaring '{}' CONSTANT would let the step fold",
                            var.get_name_ident(db).text(db)
                        ),
                        site.scope.file(db),
                        site.get_span(db),
                    ));
                }
                diag
            }
            Self::CaseLabelNotConstant {
                label,
                as_range_bound,
            } => diag()
                .message(
                    if *as_range_bound {
                        "a CASE range bound must be an integer constant"
                    } else {
                        "a CASE label must evaluate to a constant at compile time"
                    }
                    .to_string(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &label.get_span(db)).unwrap_or_default())
                .call(),
        }
    }
}
