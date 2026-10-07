// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        config::ProgConfig,
        expressions::{
            expression::{ExprKind, PrimaryExpr, VariableAccess, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::{
            pou::Pou,
            variable::{LocationArea, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        body::{ParamBinding, ScopeInference},
        expr_store::PathExprWalkStep,
        index_graphs::{effective_location, external_var_lookup},
        infer::Infer,
        oop::MethodRef,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "double-writer";

/// L0129: an output location written by more than one program instance.
struct DoubleWriter;

impl ErrorCode for DoubleWriter {
    fn code(&self) -> &'static str {
        "L0129"
    }
}

/// An output a program writes: its address, the name it was written by, and
/// the first place it is.
struct Write<'db> {
    address: String,
    name: String,
    site: CallSite<'db>,
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for config in semantic_index(db, file).configs.iter() {
        // Each output, by address, with the instances writing it in the
        // order the configuration declares them.
        let mut writers: Vec<(String, Vec<(ProgConfig<'db>, Write<'db>)>)> = Vec::new();
        for resource in config.resources(db) {
            for instance in resource.programs(db) {
                let Type::Program(program) = instance.prog_type(db).infer(db) else {
                    continue;
                };
                let mut writes = Vec::new();
                scope_writes(
                    db,
                    program.scope_id(db),
                    &mut FxHashSet::default(),
                    &mut writes,
                );
                let mut addresses = FxHashSet::default();
                for write in writes {
                    if !addresses.insert(write.address.clone()) {
                        continue;
                    }
                    match writers
                        .iter_mut()
                        .find(|(address, _)| *address == write.address)
                    {
                        Some((_, list)) => list.push((*instance, write)),
                        None => writers.push((write.address.clone(), vec![(*instance, write)])),
                    }
                }
            }
        }
        for (address, list) in writers {
            if list.len() < 2 {
                continue;
            }
            diagnostics.push(report(db, file, &address, &list));
        }
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    address: &str,
    list: &[(ProgConfig<'db>, Write<'db>)],
) -> IdeDiagnostic {
    let names: Vec<String> = list
        .iter()
        .map(|(instance, _)| format!("'{}'", instance.name(db).with_case.text(db)))
        .collect();
    let (instance, write) = &list[1];
    let shown = match write.name.as_str() {
        name if name == address => format!("'{address}'"),
        name => format!("'{name}' at {address}"),
    };
    let mut d = diag()
        .message(format!(
            "the output {shown} is written by {} program instances, {}",
            list.len(),
            names.join(" and ")
        ))
        .desc(&DoubleWriter)
        .range(hir::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
        .severity(DiagnosticSeverity::WARNING)
        .call();
    for (instance, write) in list {
        d.with_related(Related::new(
            format!("'{}' writes it here", instance.name(db).with_case.text(db)),
            write.site.scope.file(db),
            write.site.get_span(db),
        ));
    }
    d.with_note(
        "at each scan the last instance to run sets the output, and the other writes are lost"
            .to_string(),
    );
    d.with_help("write the output in one program, and send it the others' requests".to_string());
    d
}

/// Every output location `scope` writes, itself and through what it calls:
/// FUNCTIONs, METHODs, the bodies of the blocks it runs.
fn scope_writes<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    visited: &mut FxHashSet<ScopeId<'db>>,
    writes: &mut Vec<Write<'db>>,
) {
    if !visited.insert(scope) {
        return;
    }
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => f.statements(db),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.statements(db),
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return,
    };
    let body = scope.inference(db);
    statement_writes(db, body, statements, writes);
    let mut callees = Vec::new();
    for (_, resolved) in body.resolved_calls() {
        for (param, binding) in &resolved.params {
            match binding {
                ParamBinding::Values(values) if param.is_in_out(db) => {
                    for value in values {
                        if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) =
                            value.expr(db)
                        {
                            place_write(db, body, *access, writes);
                        }
                    }
                }
                ParamBinding::Output { variable, .. } => place_write(db, body, *variable, writes),
                _ => {}
            }
        }
        if let Some(callee) = callee_scope(db, resolved.callable)
            && !callees.contains(&callee)
        {
            callees.push(callee);
        }
    }
    for callee in callees {
        scope_writes(db, callee, visited, writes);
    }
}

fn statement_writes<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
    writes: &mut Vec<Write<'db>>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, .. } => place_write(db, body, *var, writes),
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                statement_writes(db, body, then.as_deref().unwrap_or_default(), writes);
                for (_, stmts) in else_if {
                    statement_writes(db, body, stmts, writes);
                }
                statement_writes(db, body, else_.as_deref().unwrap_or_default(), writes);
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    statement_writes(db, body, stmts, writes);
                }
                statement_writes(db, body, else_.as_deref().unwrap_or_default(), writes);
            }
            StmtKind::For {
                control_variable,
                body: stmts,
                ..
            } => {
                place_write(db, body, *control_variable, writes);
                statement_writes(db, body, stmts, writes);
            }
            StmtKind::While { body: stmts, .. } | StmtKind::Repeat { body: stmts, .. } => {
                statement_writes(db, body, stmts, writes)
            }
            _ => {}
        }
    }
}

/// A write to `access`, recorded when it lands in an output location: a
/// bare `%Q` address, a variable located there, a VAR_EXTERNAL naming a
/// global located there. A place written through a reference is not.
fn place_write<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    access: VariableAccess<'db>,
    writes: &mut Vec<Write<'db>>,
) {
    let site = access.as_call_site(db);
    let (location, name) = match access.kind(db) {
        VariableAccessKind::Direct(direct) => (direct, None),
        VariableAccessKind::Symbolic(begin) => {
            let Some(path) = begin.expr(db) else {
                return;
            };
            let steps = path.flatten(db);
            if steps
                .iter()
                .any(|step| matches!(step, PathExprWalkStep::Deref { .. }))
            {
                return;
            }
            let Some(root) = steps
                .first()
                .and_then(|step| body.variable_for_path_expr(step.get_expr(db)))
            else {
                return;
            };
            // A VAR_EXTERNAL names the global: its location is the global's.
            let declared = match root.kind(db) {
                VariableKind::External => external_var_lookup(db, root.get_name_ident(db)),
                _ => Some(root),
            };
            let Some(location) = declared.and_then(|var| effective_location(db, var)) else {
                return;
            };
            (
                location,
                Some(root.get_name_with_case(db).text(db).to_string()),
            )
        }
    };
    if location.area(db) != Some(LocationArea::Output) {
        return;
    }
    let address = location.to_address(db);
    writes.push(Write {
        name: name.unwrap_or_else(|| address.clone()),
        address,
        site,
    });
}

/// The scope of the body a call runs: a FUNCTION's, a METHOD's, a block's.
/// `None` through an interface, whose body only the instance knows.
pub(crate) fn callee_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
) -> Option<ScopeId<'db>> {
    match callable {
        CallableType::Function(f) => Some(f.scope_id(db)),
        CallableType::FunctionBlock(fb) => Some(fb.scope_id(db)),
        CallableType::MethodDecl(MethodRef::Declared(m)) => Some(m.scope_id(db)),
        CallableType::MethodDecl(MethodRef::Prototype(_)) => None,
    }
}
