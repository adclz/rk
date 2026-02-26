use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{expression::FuncCall, statement::Stmt},
        pous::variable::VariableDecl,
    },
    hir_ty::ty::{CallableType, Type},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ControlFlowError<'db> {
    IsVarInput {
        var: VariableDecl<'db>,
        access: CallSite<'db>,
    },
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
}

impl<'db> ErrorCode for ControlFlowError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::AssignCallableType { .. } => "E1001",
            Self::IsVarInput { .. } => "E1002",
            Self::DirectType { .. } => "E1003",
            Self::CallNonCallableType { .. } => "E1004",
            Self::ContinueOutsideLoop { .. } => "E1005",
            Self::ExitOutsideLoop { .. } => "E1006",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::AssignCallableType { .. } | Self::IsVarInput { .. } | Self::DirectType { .. } => {
                "assignment violation"
            }
            _ => "control flow violation",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ControlFlowError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::IsVarInput { var, access } => diag()
                .message(format!(
                    "{} is an input variable and can not be assigned",
                    var.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(access.get_span(db))
                .call(),
            Self::AssignCallableType { typ, access } => diag()
                .message(format!(
                    "'{}' is a callable type and can not be assigned",
                    typ.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(access.get_span(db))
                .call(),
            Self::DirectType { expr, typ } => diag()
                .message(format!(
                    "cannot use direct type '{}' here",
                    typ.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::CallNonCallableType { typ, func_call } => {
                let mut diag = diag()
                    .message(format!("'{}' is not a callable type", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(func_call.path(db).get_span(db))
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
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ExitOutsideLoop { stmt } => diag()
                .message("'EXIT' can only be used inside loops".to_string())
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
        }
    }
}
