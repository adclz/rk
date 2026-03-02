use auto_lsp::{
    default::db::file::File,
    lsp_types::{
        ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation,
    },
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, ExprKind, FuncCall, PrimaryExpr},
            statement::{CaseKind, Stmt, StmtKind},
        },
        hir_node::HirNode,
        pous::{pou::Pou, variable::VariableKind},
        scope::ScopeKind,
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{head::signature::infer_signature, infer::Infer, ty::Type},
};

use crate::walk::WalkHir;

/// Find signature help for the function call enclosing the given offset.
pub fn find_signature_help<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<SignatureHelp> {
    let func_call = find_enclosing_func_call(db, file, offset)?;
    let callable = resolve_callable(db, &func_call)?;

    let callable_name = callable.get_name_ident(db).text(db).to_string();
    let scope = callable.get_scope_id(db);
    let signature = infer_signature(db, scope);

    // Collect parameter info (Input, Output, InOut only)
    let params: Vec<(String, &str, String)> = scope
        .def_map(db)
        .local_variables
        .iter()
        .filter_map(|(_, var)| {
            let kind = var.kind(db);
            match kind {
                VariableKind::Input | VariableKind::Output | VariableKind::InOut => {
                    let name = var.name(db).text(db).to_string();
                    let kind_str = match kind {
                        VariableKind::Output => " =>",
                        _ => " :=",
                    };
                    let type_name = signature
                        .type_of_specs
                        .get(&var.spec(db))
                        .map(|t| t.type_name(db))
                        .unwrap_or_default();
                    Some((name, kind_str, type_name))
                }
                _ => None,
            }
        })
        .collect();

    // Build return type suffix
    let return_suffix = scope
        .return_type(db)
        .and_then(|spec| {
            let ty = signature.type_of_specs.get(spec)?;
            Some(format!(" : {}", ty.type_name(db)))
        })
        .unwrap_or_default();

    // Build the signature label with parameter offset tracking
    let mut label = callable_name.clone();
    label.push('(');
    let mut param_infos = Vec::with_capacity(params.len());

    for (i, (name, kind_str, type_name)) in params.iter().enumerate() {
        if i > 0 {
            label.push_str(", ");
        }
        let start = label.len() as u32;
        label.push_str(name);
        label.push_str(kind_str);
        label.push(' ');
        label.push_str(type_name);
        let end = label.len() as u32;

        param_infos.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([start, end]),
            documentation: None,
        });
    }

    label.push(')');
    label.push_str(&return_suffix);

    // Determine active parameter
    let active_param = determine_active_param(db, &func_call, offset);

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(param_infos),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    })
}

/// Find the innermost FuncCall containing the offset.
///
/// FuncCall can appear as `StmtKind::FuncCall` (statement-level) or
/// `ExprKind::PrimaryExpr(PrimaryExpr::FuncCall)` (expression-level).
/// Since statements are not in the flat node_index, we first find the
/// enclosing scope, then walk its statements recursively.
fn find_enclosing_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<FuncCall<'db>> {
    // Find the enclosing POU scope
    let scope_id = find_enclosing_scope(db, file, offset)?;

    // Get statements from the scope
    let statements = match get_scope(db, scope_id).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return None,
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return None,
    };

    // Walk statements to find FuncCall containing the offset
    let mut best: Option<FuncCall<'db>> = None;
    find_func_call_in_stmts(db, statements, offset, &mut best);
    best
}

/// Find the scope (POU/method) that contains the given offset.
fn find_enclosing_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<hir::hir_def::scope::ScopeId<'db>> {
    let sema = semantic_index(db, file);
    let mut best_scope = None;
    let mut best_size = usize::MAX;

    let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
        if let HirNode::PouDecl(_) = &node {
            let span = node.get_span(db);
            if span.start_byte <= offset && offset <= span.end_byte {
                let size = span.end_byte - span.start_byte;
                if size < best_size {
                    best_size = size;
                    best_scope = Some(node.get_scope_id(db));
                }
            }
        }
        std::ops::ControlFlow::Continue(())
    });

    best_scope
}

/// Recursively search statements for a FuncCall containing the offset.
fn find_func_call_in_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    offset: usize,
    best: &mut Option<FuncCall<'db>>,
) {
    for stmt in stmts {
        let span = stmt.get_span(db);
        if offset < span.start_byte || offset > span.end_byte {
            continue;
        }
        match stmt.stmt(db) {
            StmtKind::FuncCall(fc) => {
                *best = Some(*fc);
            }
            StmtKind::Assignment { target, .. } | StmtKind::AssignmentAttempt { target, .. } => {
                find_func_call_in_expr(db, target, offset, best);
            }
            StmtKind::If { condition, then, else_if, else_, .. } => {
                find_func_call_in_expr(db, condition, offset, best);
                if let Some(stmts) = then {
                    find_func_call_in_stmts(db, stmts, offset, best);
                }
                for (cond, stmts) in else_if {
                    find_func_call_in_expr(db, cond, offset, best);
                    find_func_call_in_stmts(db, stmts, offset, best);
                }
                if let Some(stmts) = else_ {
                    find_func_call_in_stmts(db, stmts, offset, best);
                }
            }
            StmtKind::Case { condition, cases, else_ } => {
                find_func_call_in_expr(db, condition, offset, best);
                for (kinds, stmts) in cases {
                    for kind in kinds {
                        match kind {
                            CaseKind::Expression(e) => find_func_call_in_expr(db, e, offset, best),
                            CaseKind::Subrange { lower, upper } => {
                                find_func_call_in_expr(db, lower, offset, best);
                                find_func_call_in_expr(db, upper, offset, best);
                            }
                        }
                    }
                    find_func_call_in_stmts(db, stmts, offset, best);
                }
                if let Some(stmts) = else_ {
                    find_func_call_in_stmts(db, stmts, offset, best);
                }
            }
            StmtKind::For { body, .. } | StmtKind::While { body, .. } | StmtKind::Repeat { body, .. } => {
                find_func_call_in_stmts(db, body, offset, best);
            }
            _ => {}
        }
    }
}

/// Search an expression tree for a FuncCall containing the offset.
fn find_func_call_in_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    offset: usize,
    best: &mut Option<FuncCall<'db>>,
) {
    let span = expr.get_span(db);
    if offset < span.start_byte || offset > span.end_byte {
        return;
    }
    if let ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(fc)) = expr.expr(db) {
        *best = Some(*fc);
    }
}

/// Resolve a FuncCall's target to a CallableType.
fn resolve_callable<'db>(
    db: &'db dyn WorkspaceDataBase,
    func_call: &FuncCall<'db>,
) -> Option<hir::hir_ty::ty::CallableType<'db>> {
    let target_type = func_call.path(db).infer(db);
    if target_type.is_never() {
        return None;
    }
    // infer() on a call target may return either Type::Function/FunctionBlock/MethodDecl
    // or Type::CallableType wrapping a CallableType
    match target_type {
        Type::CallableType(ct) => Some(ct),
        _ => target_type.as_callable(db),
    }
}

/// Determine which parameter is active based on cursor offset.
/// Counts how many ParamAssign spans end before the cursor.
fn determine_active_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    func_call: &FuncCall<'db>,
    offset: usize,
) -> u32 {
    let params = func_call.params(db);
    if params.is_empty() {
        return 0;
    }

    // Find which param the cursor is in or after
    let mut active = 0u32;
    for (i, param) in params.iter().enumerate() {
        let span = param.get_span(db);
        if offset >= span.start_byte {
            active = i as u32;
        }
    }

    active
}
