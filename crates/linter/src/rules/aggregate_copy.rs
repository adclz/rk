// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::expressions::{
        expression::{ParamAssignKind, VariableAccess},
        statement::Stmt,
    },
    hir_ty::{
        body::ScopeInference,
        infer::Infer,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "aggregate-copy";

/// L0214: an ARRAY, a STRUCT or a FUNCTION_BLOCK or CLASS instance copied
/// whole, by an assignment or a call binding. The copy takes every element
/// and field, nested ones included, and costs as much as the type is large.
/// A STRING copies its text only and is not reported. A VAR_IN_OUT shares
/// the value instead of copying it.
struct AggregateCopy;

impl ErrorCode for AggregateCopy {
    fn code(&self) -> &'static str {
        "L0214"
    }
}

/// `target := value`, where the target is an aggregate.
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmt: Stmt<'db>,
    target: VariableAccess<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let Some(aggregate) = Aggregate::of(db, body.type_of_variable_access_adjusted(target)) else {
        return;
    };
    diagnostics.push(report(
        format!("the assignment copies {}", aggregate.what(db)),
        hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db)),
    ));
}

/// The bindings of every call in the body: `in := value` copies the argument
/// into the input, `out => target` copies the output into the target.
pub fn check_calls<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for call in body.calls() {
        for param in call.params(db) {
            let Some(formal) = body.variable_for_param(*param) else {
                continue;
            };
            // A VAR_IN_OUT binds by reference: nothing is copied.
            if formal.is_in_out(db) {
                continue;
            }
            let Some(aggregate) = Aggregate::of(db, formal.spec(db).infer(db)) else {
                continue;
            };
            let formal_name = formal.get_name_with_case(db).text(db);
            let message = match param.kind(db) {
                ParamAssignKind::NonFormal { .. } | ParamAssignKind::FormalInput { .. } => {
                    format!(
                        "the argument copies {} into '{formal_name}'",
                        aggregate.what(db)
                    )
                }
                ParamAssignKind::FormalOutput { .. } => format!(
                    "the binding copies {} out of '{formal_name}'",
                    aggregate.what(db)
                ),
            };
            diagnostics.push(report(
                message,
                hir::denormalize(db, param.get_scope_id(db).file(db), &param.get_span(db)),
            ));
        }
    }
}

fn report(message: String, range: Option<auto_lsp::lsp_types::Range>) -> IdeDiagnostic {
    let mut d = diag()
        .message(message)
        .desc(&AggregateCopy)
        .range(range.unwrap_or_default())
        .severity(DiagnosticSeverity::INFORMATION)
        .call();
    d.with_note(
        "a copy takes every element and field, nested ones included, and costs as much as the type is large"
            .to_string(),
    );
    d.with_help("to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it".to_string());
    d
}

/// What an assignment copies whole, named as written.
enum Aggregate<'db> {
    Instance(Type<'db>),
    Struct(Type<'db>),
    Array(Type<'db>),
}

impl<'db> Aggregate<'db> {
    fn of(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Self> {
        match ty.normalize(db) {
            instance @ (Type::FunctionBlock(_) | Type::Class(_)) => Some(Self::Instance(instance)),
            // A field or an element of instance type reads as the callable
            // it is an instance of.
            Type::CallableType(CallableType::FunctionBlock(fb)) => {
                Some(Self::Instance(Type::FunctionBlock(fb)))
            }
            // Named as written: normalized, a `Pt` is `STRUCT`.
            Type::Struct(_) => Some(Self::Struct(ty)),
            Type::Array(_) => Some(Self::Array(ty)),
            _ => None,
        }
    }

    /// What the copy takes, for the message.
    fn what(&self, db: &'db dyn WorkspaceDataBase) -> String {
        match self {
            Self::Instance(ty) => format!("the whole '{}' instance", ty.type_name(db)),
            Self::Struct(ty) => format!("the whole '{}' structure", ty.type_name(db)),
            Self::Array(ty) => {
                let name = ty.type_name(db);
                if name.starts_with("ARRAY") {
                    format!("the whole '{name}'")
                } else {
                    format!("the whole '{name}' array")
                }
            }
        }
    }
}
