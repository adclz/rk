use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{infer::Infer, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "instance-in-function";

/// L0119: a FUNCTION or METHOD holding or returning a FUNCTION_BLOCK or CLASS
/// instance. Both are stateless: an instance one holds starts over at every
/// call, so a timer in it never expires, and one it returns is made afresh by
/// each call. A VAR_IN_OUT or VAR_EXTERNAL instance lives elsewhere and is not
/// reported, nor is a `{test}` FUNCTION's, which the runner calls once.
struct InstanceInFunction;

impl ErrorCode for InstanceInFunction {
    fn code(&self) -> &'static str {
        "L0119"
    }

    fn description(&self) -> &'static str {
        "instance in a stateless POU"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let (callable, variables, ret) = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            if hir::hir_def::pous::pragma::is_test(db, f.pragmas(db)) {
                return;
            }
            ("FUNCTION", f.variables(db), f.return_type(db))
        }
        ScopeKind::MethodDecl(m) => ("METHOD", m.variables(db), m.return_type(db)),
        _ => return,
    };
    let file = scope.file(db);
    for var in variables {
        if var.is_in_out(db) || var.is_external(db) {
            continue;
        }
        let Some(kind) = instance_kind(db, var.spec(db).infer(db)) else {
            continue;
        };
        let mut d = diag()
            .message(format!(
                "'{}' holds a {kind} instance, which starts over at every call of the {callable}",
                var.get_name_ident(db).text(db)
            ))
            .desc(&InstanceInFunction)
            .range(hir::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call();
        d.with_note("FUNCTIONs and METHODs are stateless".to_string());
        diagnostics.push(d);
    }
    if let Some(ret) = ret
        && let Some(kind) = instance_kind(db, ret.infer(db))
    {
        let mut d = diag()
            .message(format!(
                "the {callable} returns a {kind} instance, made afresh by each call"
            ))
            .desc(&InstanceInFunction)
            .range(hir::denormalize(db, file, &ret.get_span(db)).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call();
        d.with_note("FUNCTIONs and METHODs are stateless".to_string());
        diagnostics.push(d);
    }
}

/// `FUNCTION_BLOCK` or `CLASS` when `ty` is an instance of one, or an array
/// of them.
fn instance_kind<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<&'static str> {
    let mut ty = ty.normalize(db);
    while let Type::Array(array) = ty {
        ty = array.of_type(db).infer(db).normalize(db);
    }
    match ty {
        Type::FunctionBlock(_) => Some("FUNCTION_BLOCK"),
        Type::Class(_) => Some("CLASS"),
        _ => None,
    }
}
