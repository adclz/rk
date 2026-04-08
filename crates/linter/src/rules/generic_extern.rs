use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            spec::SpecKind,
            statement::{Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "generic-extern";

/// L0120: extern pragma references a generic parameter — the host must provide
/// an implementation for each concrete type the ANY_* constraint accepts.
struct GenericExtern;

impl ErrorCode for GenericExtern {
    fn code(&self) -> &'static str {
        "L0120"
    }

    fn description(&self) -> &'static str {
        "generic extern function"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let scope_data = get_scope(db, scope);

    let (statements, variables, pou_name): (&[Stmt<'db>], &[VariableDecl<'db>], Option<&str>) =
        match scope_data.kind {
            ScopeKind::Pou(Pou::Function(f)) => {
                (f.statements(db), f.variables(db), Some(f.name(db).text(db)))
            }
            ScopeKind::MethodDecl(m) => {
                (m.stmts(db), m.variables(db), Some(m.name(db).text(db)))
            }
            _ => return,
        };

    let file = scope.file(db);

    for stmt in statements {
        let StmtKind::ExternPragma(extern_decl) = stmt.stmt(db) else {
            continue;
        };

        // Check each param
        for param in &extern_decl.params {
            let param_name = param.ident.text(db);
            if let Some((type_name, var)) = find_generic_var(db, param_name, variables) {
                let mut d = diag()
                    .message(format!(
                        "extern parameter '{param_name}' has generic type {type_name}: \
                         the host must provide an implementation for each concrete type variant"
                    ))
                    .desc(&GenericExtern)
                    .range(param.get_span(db))
                    .severity(DiagnosticSeverity::INFORMATION)
                    .call();
                d.with_related(Related::new(
                    format!("'{param_name}' is declared as {type_name}"),
                    file,
                    var.get_name_span(db),
                ));
                diagnostics.push(d);
            }
        }

        // Check result
        if let Some(result) = &extern_decl.result {
            let result_name = result.ident.text(db);

            // Result may be the POU name (return type)
            if pou_name.is_some_and(|n| n == result_name) {
                if let Some(ret_spec) = scope.return_type(db) {
                    if let SpecKind::Simple(e) = ret_spec.kind(db) {
                        if e.is_any() {
                            let type_name = e.type_name();
                            let mut d = diag()
                                .message(format!(
                                    "extern result '{result_name}' has generic return type {type_name}: \
                                     the host must provide an implementation for each concrete type variant"
                                ))
                                .desc(&GenericExtern)
                                .range(result.get_span(db))
                                .severity(DiagnosticSeverity::INFORMATION)
                                .call();
                            d.with_related(Related::new(
                                format!("return type is {type_name}"),
                                file,
                                ret_spec.get_span(db),
                            ));
                            diagnostics.push(d);
                        }
                    }
                }
            } else if let Some((type_name, var)) = find_generic_var(db, result_name, variables) {
                let mut d = diag()
                    .message(format!(
                        "extern result '{result_name}' has generic type {type_name}: \
                         the host must provide an implementation for each concrete type variant"
                    ))
                    .desc(&GenericExtern)
                    .range(result.get_span(db))
                    .severity(DiagnosticSeverity::INFORMATION)
                    .call();
                d.with_related(Related::new(
                    format!("'{result_name}' is declared as {type_name}"),
                    file,
                    var.get_name_span(db),
                ));
                diagnostics.push(d);
            }
        }
    }
}

/// Find a variable by name and return its ANY_* type name and the variable itself.
fn find_generic_var<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: &str,
    variables: &[VariableDecl<'db>],
) -> Option<(&'static str, VariableDecl<'db>)> {
    for var in variables {
        if var.name(db).text(db) == name {
            return match var.spec(db).kind(db) {
                SpecKind::Simple(e) if e.is_any() => Some((e.type_name(), *var)),
                SpecKind::Into(_) => Some(("INTO", *var)),
                _ => None,
            };
        }
    }
    None
}
