use auto_lsp::{
    default::db::file::File,
    lsp_types::{ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation},
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::TaskConfig,
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

/// Find signature help for the function call or TASK configuration enclosing the given offset.
pub fn find_signature_help(
    db: &dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<SignatureHelp> {
    if let Some(help) = find_func_call_signature_help(db, file, offset) {
        return Some(help);
    }

    find_task_signature_help(db, file, offset)
}

/// Find signature help for a TASK configuration init at the given offset.
fn find_task_signature_help(
    db: &dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<SignatureHelp> {
    let task = find_enclosing_task(db, file, offset)?;

    // Verify cursor is inside the parentheses (init section)
    let source = file.document(db).as_str();
    let span = task.get_span(db);
    let task_text = &source[span.start_byte..span.end_byte];
    let paren_offset = task_text.find('(')?;
    let paren_abs = span.start_byte + paren_offset;
    if offset <= paren_abs {
        return None;
    }

    // Build the static TASK signature label with parameter offsets
    let params_data: &[(&str, &str)] = &[
        ("SINGLE", "DataSource"),
        ("INTERVAL", "DataSource"),
        ("PRIORITY", "UINT"),
    ];

    let mut label = String::from("TASK(");
    let mut param_infos = Vec::with_capacity(params_data.len());

    for (i, (name, ty)) in params_data.iter().enumerate() {
        if i > 0 {
            label.push_str(", ");
        }
        let start = label.len() as u32;
        label.push_str(name);
        label.push_str(" := ");
        label.push_str(ty);
        let end = label.len() as u32;

        param_infos.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([start, end]),
            documentation: None,
        });
    }
    label.push(')');

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(param_infos),
            active_parameter: None,
        }],
        active_signature: Some(0),
        active_parameter: None,
    })
}

/// Find signature help for a function call enclosing the given offset.
fn param_info<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: &hir::hir_def::pous::variable::VariableDecl<'db>,
    signature: &hir::hir_ty::head::signature::Signature<'db>,
) -> Option<(String, &'static str, String)> {
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
}

fn find_func_call_signature_help(
    db: &dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<SignatureHelp> {
    let func_call = find_enclosing_func_call(db, file, offset)?;
    let callable = resolve_callable(db, &func_call)?;

    // An overloaded name has several declarations behind it, and the editor
    // cycles through them. Which one the call resolved to is already decided:
    // `callable` is what inference recorded, so this only lists its siblings.
    let (candidates, active_signature) = match callable {
        hir::hir_ty::ty::CallableType::Function(f) => {
            let set = hir::hir_ty::resolver::name::overload_set(db, f);
            match set.iter().position(|c| *c == f) {
                Some(active) => (
                    set.into_iter()
                        .map(hir::hir_ty::ty::CallableType::Function)
                        .collect(),
                    active as u32,
                ),
                None => (vec![callable], 0),
            }
        }
        other => (vec![other], 0),
    };

    let active_parameter = determine_active_param(db, &func_call, offset);

    Some(SignatureHelp {
        signatures: candidates
            .into_iter()
            .map(|c| signature_of(db, c, active_parameter))
            .collect(),
        active_signature: Some(active_signature),
        active_parameter: Some(active_parameter),
    })
}

/// One candidate's label, carrying the offsets an editor highlights the
/// active parameter by.
fn signature_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: hir::hir_ty::ty::CallableType<'db>,
    active_parameter: u32,
) -> SignatureInformation {
    let callable_name = callable.get_name_ident(db).text(db).to_string();
    let scope = callable.get_scope_id(db);
    let signature = infer_signature(db, scope);

    // Collect parameter info (Input, Output, InOut only). For an FB the list
    // is the flattened EXTENDS view, matching what the call site binds; an
    // inherited member's type lives in its OWNER's signature, not this one.
    let params: Vec<(String, &str, String)> = match callable {
        hir::hir_ty::ty::CallableType::FunctionBlock(fb) => {
            hir::hir_ty::head::inheritance::instance_members(
                db,
                hir::hir_def::pous::pou::Pou::FunctionBlock(fb),
            )
            .iter()
            .filter_map(|m| {
                let owner_sig = infer_signature(db, m.owner.get_scope_id(db));
                param_info(db, &m.var, owner_sig)
            })
            .collect()
        }
        _ => scope
            .def_map(db)
            .local_variables
            .iter()
            .filter_map(|(_, var)| param_info(db, var, signature))
            .collect(),
    };

    // Build return type suffix
    let return_suffix = scope
        .return_type(db)
        .and_then(|spec| {
            let ty = signature.type_of_specs.get(spec)?;
            Some(format!(" : {}", ty.type_name(db)))
        })
        .unwrap_or_default();

    // Build the signature label with parameter offset tracking
    let mut label = callable_name;
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

    SignatureInformation {
        label,
        documentation: None,
        parameters: Some(param_infos),
        active_parameter: Some(active_parameter),
    }
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
        // Every node that holds statements of its own. A PROGRAM was missing
        // entirely, and a METHOD's body was read as its POU's, so a call in
        // either got no help at all.
        if matches!(
            node,
            HirNode::PouDecl(_) | HirNode::Program(_) | HirNode::MethodRef(_)
        ) {
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

/// Find the smallest TaskConfig node containing the given offset.
fn find_enclosing_task<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<TaskConfig<'db>> {
    let sema = semantic_index(db, file);
    let mut best: Option<TaskConfig<'db>> = None;
    let mut best_size = usize::MAX;

    let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
        if let HirNode::Task(t) = node {
            let span = node.get_span(db);
            if span.start_byte <= offset && offset <= span.end_byte {
                let size = span.end_byte - span.start_byte;
                if size < best_size {
                    best_size = size;
                    best = Some(t);
                }
            }
        }
        std::ops::ControlFlow::Continue(())
    });

    best
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
            StmtKind::Assignment { target, .. } => {
                find_func_call_in_expr(db, target, offset, best);
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
                ..
            } => {
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
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
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
            StmtKind::For { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. } => {
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

/// Which parameter the cursor is writing: the separators it sits behind.
///
/// Counted in the source rather than over the argument nodes, because an
/// argument only half typed is not the node it will become. `f(a |)` parses
/// `a` as a whole argument, so counting finished nodes jumped to the second
/// parameter while the user was still naming the first.
fn determine_active_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    func_call: &FuncCall<'db>,
    offset: usize,
) -> u32 {
    let path = func_call.path(db);
    let document = path.get_scope_id(db).file(db).document(db);
    // From the callee's name to the cursor: what the user has written into
    // the argument list so far, and nothing beyond it.
    let Some(window) = document.as_str().get(path.get_span(db).end_byte..offset) else {
        return 0;
    };
    let Some(open) = window.find('(') else {
        return 0;
    };
    let written = &window[open + 1..];

    let mut depth = 0i32;
    let mut in_string = false;
    let mut commas = 0u32;
    for c in written.chars() {
        match c {
            '\'' => in_string = !in_string,
            _ if in_string => {}
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => commas += 1,
            _ => {}
        }
    }
    commas
}
