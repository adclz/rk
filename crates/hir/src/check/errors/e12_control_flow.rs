// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e00_syntax::first_bytes;
use crate::hir_def::expressions::statement::{Stmt, StmtKind};
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
    /// A `__TRY` with neither a `__CATCH` nor a `__FINALLY`: nothing of it
    /// is left but its body.
    TryWithoutClause {
        stmt: Stmt<'db>,
    },
    /// A `RETURN`, an `EXIT` or a `CONTINUE` that leaves a `__FINALLY`,
    /// which runs with an exception pending as well: the jump would drop
    /// it.
    JumpLeavesFinally {
        stmt: Stmt<'db>,
    },
    /// A `__CATCH` written after the `__FINALLY`.
    CatchAfterFinally {
        span: auto_lsp::tree_sitter::Range,
    },
    /// A second `__CATCH`, or a second `__FINALLY`: a `__TRY` has one of
    /// each. `span` is the clause, `first` the one it comes after.
    ClauseWrittenTwice {
        span: auto_lsp::tree_sitter::Range,
        first: auto_lsp::tree_sitter::Range,
    },
    /// A FOR control variable that is not a bare identifier. IEC 61131-3's
    /// grammar says `control_variable ::= identifier`, so `r.i`, `a[k]` and
    /// `p^` cannot be counters. (A bare name
    /// that RESOLVES to an FB/PROGRAM member is fine; the restriction is on
    /// the syntax, not on where the variable lives.)
    ForControlNotAVariable {
        access: CallSite<'db>,
    },
    /// A FOR control variable that is not an integer: IEC 61131-3 counts in
    /// ANY_INT. A BOOL, a REAL, a TIME or a bit string was accepted and
    /// looped over values a counter does not have.
    ForControlNotInteger {
        access: CallSite<'db>,
        ty: crate::hir_ty::ty::Type<'db>,
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
    /// A CASE selector of a type CASE does not branch on: it takes an
    /// integer, a bit string, a CHAR, an enum or a STRING. A REAL one built
    /// an invalid module, and a BOOL or TIME one was refused label by label.
    CaseSelectorNotSupported {
        selector: CallSite<'db>,
        ty: crate::hir_ty::ty::Type<'db>,
    },
    /// A CASE range whose lower bound is above its upper: no value is in
    /// it, so its arm never runs.
    CaseRangeEmpty {
        range: CallSite<'db>,
        lower: i128,
        upper: i128,
        /// The bounds are code points, shown as the characters written.
        chars: bool,
    },
}

impl<'db> ErrorCode for ControlFlowError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::ExitOutsideLoop { .. } => "E1201",
            Self::ContinueOutsideLoop { .. } => "E1202",
            Self::TryWithoutClause { .. } => "E1209",
            Self::JumpLeavesFinally { .. } => "E1210",
            Self::CatchAfterFinally { .. } => "E1211",
            Self::ClauseWrittenTwice { .. } => "E1212",
            Self::ForControlNotAVariable { .. } => "E1203",
            Self::ForControlNotInteger { .. } => "E1206",
            Self::ForStepInvalid { .. } => "E1204",
            Self::CaseLabelNotConstant { .. } => "E1205",
            Self::CaseSelectorNotSupported { .. } => "E1207",
            Self::CaseRangeEmpty { .. } => "E1208",
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
                .message("'EXIT' is outside a loop".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ContinueOutsideLoop { stmt } => diag()
                .message("'CONTINUE' is outside a loop".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::TryWithoutClause { stmt } => {
                let keyword = first_bytes(&stmt.get_span(db), "__TRY".len());
                let mut diag = diag()
                    .message("this __TRY has neither a __CATCH nor a __FINALLY".to_string())
                    .range(crate::denormalize(db, file, &keyword).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_note(
                    "a __TRY's __CATCH handles what its body raises, and its __FINALLY runs on \
                     the way out"
                        .to_string(),
                );
                diag.with_help("add a __CATCH, or remove the __TRY".to_string());
                diag
            }
            Self::JumpLeavesFinally { stmt } => {
                let jump = match stmt.stmt(db) {
                    StmtKind::Exit => "EXIT",
                    StmtKind::Continue => "CONTINUE",
                    _ => "RETURN",
                };
                let mut diag = diag()
                    .message(format!("'{jump}' leaves the __FINALLY"))
                    .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_note(
                    "a __FINALLY also runs while an exception leaves its __TRY, and a jump out \
                     of it would end that exception with no __CATCH"
                        .to_string(),
                );
                diag.with_help(format!("move the '{jump}' after __END_TRY"));
                diag
            }
            Self::CatchAfterFinally { span } => {
                let keyword = first_bytes(span, "__CATCH".len());
                let mut diag = diag()
                    .message("this __CATCH comes after the __FINALLY".to_string())
                    .range(crate::denormalize(db, file, &keyword).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_note(
                    "the clauses of a __TRY are its __CATCH, then its __FINALLY".to_string(),
                );
                diag.with_help("move the __CATCH before the __FINALLY".to_string());
                diag
            }
            Self::ClauseWrittenTwice { span, first } => {
                // Either clause starts with its keyword.
                let source = file.document(db).as_str();
                let keyword = match source.get(span.start_byte..span.start_byte + 3) {
                    Some(start) if start.eq_ignore_ascii_case("__C") => "__CATCH",
                    _ => "__FINALLY",
                };
                let mut diag = diag()
                    .message(format!("this is the second {keyword} of the __TRY"))
                    .range(
                        crate::denormalize(db, file, &first_bytes(span, keyword.len()))
                            .unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_related(Related::new(
                    format!("the first {keyword} is here"),
                    file,
                    first_bytes(first, keyword.len()),
                ));
                diag.with_note(match keyword {
                    "__CATCH" => "a __TRY has one __CATCH, which takes every exception".to_string(),
                    _ => "a __TRY has one __FINALLY".to_string(),
                });
                diag.with_help(format!("write one {keyword} that does both"));
                diag
            }
            Self::ForControlNotAVariable { access } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is not a plain variable",
                        access.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_help(
                    "count in a plain variable and assign it where it is needed inside the loop"
                        .to_string(),
                );
                diag
            }
            Self::ForControlNotInteger { access, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is '{}', not an integer",
                        access.to_string(db),
                        ty.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_help(
                    "declare the counter as SINT, INT, DINT or LINT, or an unsigned one"
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
                            "the step is not a compile-time constant"
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
                            "'{}' is declared here, not CONSTANT",
                            var.get_name_with_case(db).text(db)
                        ),
                        site.scope.file(db),
                        var.get_name_span(db),
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
                        "the bound is not an integer constant"
                    } else {
                        "the label is not a compile-time constant"
                    }
                    .to_string(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &label.get_span(db)).unwrap_or_default())
                .call(),
            Self::CaseSelectorNotSupported { selector, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "CASE cannot branch on '{}' of type '{}'",
                        selector.to_string(db),
                        ty.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &selector.get_span(db)).unwrap_or_default())
                    .call();
                // A BOOL is a bit, not a bit string: its two values are an IF.
                diag.with_note(if ty.normalize(db).is_boolean() {
                    "a BOOL has two values".to_string()
                } else {
                    "CASE branches on an integer, a bit string, a CHAR, an enum or a STRING"
                        .to_string()
                });
                diag.with_help("branch with IF".to_string());
                diag
            }
            Self::CaseRangeEmpty {
                range,
                lower,
                upper,
                chars,
            } => {
                let shown = |bound: i128| match u32::try_from(bound).ok().and_then(char::from_u32) {
                    Some(c) if *chars => format!("'{c}'"),
                    _ => bound.to_string(),
                };
                let mut diag = diag()
                    .message(format!(
                        "CASE range {}..{} is empty",
                        shown(*lower),
                        shown(*upper)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &range.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "the lower bound is above the upper, so the arm never runs".to_string(),
                );
                diag.with_help("swap the bounds".to_string());
                diag
            }
        }
    }
}
