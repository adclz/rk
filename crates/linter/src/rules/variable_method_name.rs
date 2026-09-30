use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::head::inheritance::{MethodRef, instance_members},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "variable-method-name";

/// L0120: a variable and a method of an FB or class share a name, here or
/// across EXTENDS. It is legal, as in CODESYS and TwinCAT: inside the block
/// the name is the variable (`THIS` included), and from outside an instance
/// it is the method when the variable is the block's own (VAR, VAR_TEMP,
/// VAR_EXTERNAL). Both vendors advise against it, so warn.
struct VariableMethodName;

impl ErrorCode for VariableMethodName {
    fn code(&self) -> &'static str {
        "L0120"
    }

    fn description(&self) -> &'static str {
        "a variable and a method share a name"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let (pou, own): (Pou<'db>, &[VariableDecl<'db>]) = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou @ Pou::FunctionBlock(fb)) => (pou, fb.variables(db)),
        ScopeKind::Pou(pou @ Pou::Class(cl)) => (pou, cl.variables(db)),
        _ => return,
    };
    let methods = scope
        .method_declarations(db)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let members = hir::hir_ty::oop::class_members(db, pou);
    let inherited_vars: Vec<VariableDecl<'db>> = instance_members(db, pou)
        .iter()
        .filter(|m| m.owner != pou)
        .map(|m| m.var)
        .collect();
    let file = scope.file(db);
    let same = |a: hir::hir_def::interned::identifier::Ident,
                b: hir::hir_def::interned::identifier::Ident| { a == b };

    for var in own {
        let name = var.get_name_ident(db);
        if let Some(method) = methods.iter().find(|m| same(m.name(db), name)) {
            // Reported at the method, which is written second in the block.
            diagnostics.push(report(
                db,
                file,
                MethodRef::Declared(*method),
                *var,
                false,
                "",
            ));
        } else if let Some(method) = members.methods.get(&name).filter(|m| m.owner != pou) {
            diagnostics.push(report(db, file, method.method, *var, true, " it inherits"));
        }
    }
    for method in methods {
        let name = method.name(db);
        if own.iter().any(|v| same(v.get_name_ident(db), name)) {
            continue;
        }
        if let Some(var) = inherited_vars
            .iter()
            .find(|v| same(v.get_name_ident(db), name))
        {
            diagnostics.push(report(
                db,
                file,
                MethodRef::Declared(*method),
                *var,
                false,
                " it inherits",
            ));
        }
    }
}

/// The warning at the variable when `at_var`, at the method otherwise.
fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
    method: MethodRef<'db>,
    var: VariableDecl<'db>,
    at_var: bool,
    inherits: &str,
) -> IdeDiagnostic {
    let var_name = var.get_name_with_case(db).text(db);
    let method_name = method.get_name_with_case(db).text(db);
    let (message, span, related) = match at_var {
        true => (
            format!("variable '{var_name}' has the name of the method '{method_name}'{inherits}"),
            var.get_name_span(db),
            Related::new(
                format!("method '{method_name}' is declared here"),
                method.get_scope_id(db).file(db),
                method.get_name_span(db),
            ),
        ),
        false => (
            format!("method '{method_name}' has the name of the variable '{var_name}'{inherits}"),
            method.get_name_span(db),
            Related::new(
                format!("variable '{var_name}' is declared here"),
                var.get_scope_id(db).file(db),
                var.get_name_span(db),
            ),
        ),
    };
    let mut d = diag()
        .message(message)
        .desc(&VariableMethodName)
        .range(hir::denormalize(db, file, &span).unwrap_or_default())
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_related(related);
    d.with_note(format!(
        "inside the block, `{var_name}` and `THIS.{var_name}` are the variable"
    ));
    d
}
