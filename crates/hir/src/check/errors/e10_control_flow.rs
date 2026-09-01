use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{FuncCall, PathExpr},
            statement::Stmt,
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{
        body::NullState,
        ty::{CallableType, Type},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ControlFlowError<'db> {
    AssignCallableType {
        typ: CallableType<'db>,
        access: CallSite<'db>,
    },
    DirectType {
        typ: Type<'db>,
        expr: CallSite<'db>,
    },
    CallNonCallableType {
        typ: Type<'db>,
        func_call: FuncCall<'db>,
    },
    ContinueOutsideLoop {
        stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        stmt: Stmt<'db>,
    },
    /// A FOR control variable that is not a bare identifier. IEC 61131-3's
    /// grammar says `control_variable ::= identifier`, so `r.i`, `a[k]` and
    /// `p^` cannot be counters — and other toolchains rejects them too. (A bare name
    /// that RESOLVES to an FB/PROGRAM member is fine; the restriction is on
    /// the syntax, not on where the variable lives.)
    ForControlNotAVariable {
        access: CallSite<'db>,
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
    DerefPossiblyNull {
        var: VariableDecl<'db>,
        expr: PathExpr<'db>,
        state: NullState<'db>,
    },
    AssignToConstant {
        access: CallSite<'db>,
    },
}

impl<'db> ErrorCode for ControlFlowError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::AssignCallableType { .. } => "E0226",
            Self::DirectType { .. } => "E0228",
            Self::CallNonCallableType { .. } => "E0229",
            Self::ContinueOutsideLoop { .. } => "E1001",
            Self::ExitOutsideLoop { .. } => "E1002",
            Self::DerefPossiblyNull { .. } => "E1003",
            Self::AssignToConstant { .. } => "E1004",
            Self::ForControlNotAVariable { .. } => "E1005",
            Self::CaseLabelNotConstant { .. } => "E1006",
            Self::ForStepInvalid { .. } => "E1007",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::AssignCallableType { .. }
            | Self::DirectType { .. }
            | Self::CallNonCallableType { .. }
            | Self::AssignToConstant { .. } => "semantic violation",
            Self::DerefPossiblyNull { .. } => "possibly null dereference",
            _ => "control flow violation",
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
            Self::AssignCallableType { typ, access } => diag()
                .message(format!(
                    "'{}' is a callable type and can not be assigned",
                    typ.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                .call(),
            Self::DirectType { expr, typ } => diag()
                .message(format!(
                    "cannot use direct type '{}' here",
                    typ.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::CallNonCallableType { typ, func_call } => {
                let mut diag = diag()
                    .message(format!("'{}' is not a callable type", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                if let Type::FunctionBlock(_) = typ {
                    diag.with_note(
                        "to call a FUNCTION_BLOCK, you need to instantiate it first.".into(),
                    );
                }

                diag
            }
            Self::ContinueOutsideLoop { stmt } => diag()
                .message("'CONTINUE' can only be used inside loops".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
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
            Self::ExitOutsideLoop { stmt } => diag()
                .message("'EXIT' can only be used inside loops".to_string())
                .range(crate::denormalize(db, file, &stmt.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::AssignToConstant { access } => diag()
                .message("cannot assign to constant type".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                .call(),
            Self::DerefPossiblyNull { var, expr, state } => {
                let name = var.get_name_ident(db).text(db);
                // The related span may belong to ANOTHER variable: a state
                // propagates through `ptr := ptr_2`, so name whichever
                // variable the span actually declares or assigns.
                let (message, related_msg, site) = match state {
                    NullState::Uninitialized(origin) => {
                        let from = origin.var.get_name_ident(db).text(db);
                        (
                            format!("dereference of reference '{name}' which is never initialized"),
                            format!("'{from}' declared without initializer here"),
                            &origin.site,
                        )
                    }
                    NullState::Null(origin) => {
                        let from = origin.var.get_name_ident(db).text(db);
                        (
                            format!("dereference of reference '{name}' which is null"),
                            format!("'{from}' set to NULL here"),
                            &origin.site,
                        )
                    }
                    _ => unreachable!(),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    related_msg,
                    site.scope.file(db),
                    site.get_span(db),
                ));
                diag
            }
        }
    }
}
