use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::expressions::expression::{PathExprKind, VariableAccess, VariableAccessKind},
    hir_ty::{body::BodyInferenceResult, head::signature::infer_signature, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "external-mutation";

/// L0117: a variable of a function block or class instance is modified from outside.
/// Instances should own their own data.
struct ExternalMutation;

impl ErrorCode for ExternalMutation {
    fn code(&self) -> &'static str {
        "L0117"
    }

    fn description(&self) -> &'static str {
        "external instance mutation"
    }
}

/// Called by the stmt_visitor for each assignment. Check if the LHS is
/// a field access on a FB/CLASS instance (e.g. `fb.x := 42`).
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var: VariableAccess<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
        return;
    };
    // THIS.field.x is internal OOP access, not external mutation
    if begin.invocation(db).is_some() {
        return;
    }

    let Some(outer_path) = begin.expr(db) else {
        return;
    };

    // Check if the outermost path step is a field access
    let PathExprKind::Field(field_expr) = outer_path.expr(db) else {
        return;
    };

    // The inner path is the thing before the dot (e.g. `fb` in `fb.x`)
    let inner_path = field_expr.path;

    // Check what type the inner path resolves to
    let Some(inner_type) = body.type_of_path_expr.get(&inner_path) else {
        return;
    };

    // Only flag if the inner type is a FB or CLASS instance variable
    let instance_name = match inner_type {
        Type::Variable((decl, _)) => {
            let sig = infer_signature(db, decl.get_scope_id(db));
            let var_type = sig.type_of_specs.get(&decl.spec(db));
            match var_type {
                Some(Type::FunctionBlock(_) | Type::Class(_)) => decl.get_name_ident(db).text(db),
                _ => return,
            }
        }
        _ => return,
    };

    let field_name = outer_path.ident(db).ident.text(db);
    diagnostics.push(
        diag()
            .message(format!(
                "direct mutation of '{instance_name}.{field_name}' - instances should own their data"
            ))
            .desc(&ExternalMutation)
            .range(hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_span(db)).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}
