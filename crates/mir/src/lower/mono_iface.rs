//! Phase B: interface-parameter monomorphization.
//!
//! Design 1 guarantees an interface only ever appears as a direct `VAR_INPUT` /
//! `VAR_IN_OUT` parameter, so at every call site passing an interface argument
//! the concrete implementer is statically known. We specialize the callee
//! FUNCTION per concrete binding (`drive` -> `drive$Worker`) and rewrite the
//! call to it; inside the specialization `dev.Method()` lowers to a direct
//! `Worker#Method` (see `lower_expr::resolve_method_call`).

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use hir::hir_def::expressions::expression::{
    Expr, ExprKind, FuncCall, ParamAssignKind, PrimaryExpr, VariableAccessKind,
};
use hir::hir_def::expressions::invocation::InvocationKind;
use hir::hir_def::expressions::statement::{Stmt, StmtKind};
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::variable::{VariableDecl, VariableKind};
use hir::hir_def::scope::{ScopeId, ScopeKind};
use hir::hir_def::semantic_index::get_scope;
use hir::hir_ty::body::infer_body;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::ty::{CallableType, Type};

use super::monomorphize::qualified_pou_ident;

/// Call-site `FuncCall` → the mangled specialization it must call,
/// threaded into every body.
pub type IfaceCallRewrites<'db> = FxHashMap<FuncCall<'db>, Ident>;

/// One specialization of a function on the concrete implementers bound to its
/// interface parameters (e.g. `drive` with `dev -> Worker` => `drive$Worker`).
pub struct IfaceInstance<'db> {
    pub func: Function<'db>,
    /// interface param name -> concrete implementer POU
    pub iface_subs: FxHashMap<Ident, Pou<'db>>,
    pub mangled_name: Ident,
    /// Rewrites for calls inside this specialization's body: a forwarded
    /// interface param resolves to a different transitive specialization per
    /// binding.
    pub call_rewrites: FxHashMap<FuncCall<'db>, Ident>,
}

/// Walk every POU body; for each call to a function with interface
/// parameters, record the specialization it needs and the call rewrite.
/// Seed from the generically emitted bodies, then process each
/// specialization with its own substitution active, to fixpoint.
pub fn collect_iface_instantiations<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
) -> (Vec<IfaceInstance<'db>>, FxHashMap<FuncCall<'db>, Ident>) {
    // Canonical key: (qualified func name, sorted [(param name, qualified concrete)]).
    let mut by_canonical: FxHashMap<(Ident, Vec<(Ident, Ident)>), Ident> = FxHashMap::default();
    let mut instances: Vec<IfaceInstance<'db>> = Vec::new();
    // Rewrites for the generic bodies; specializations own theirs.
    let mut global_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
    let no_subs: FxHashMap<Ident, Pou<'db>> = FxHashMap::default();

    // --- Seed ---
    for (pou, _) in all_pous {
        // Walk every body of this POU — its own statements plus each method's —
        // for calls in BOTH expression and statement context.
        match pou {
            Pou::Function(f) => {
                // Interface-param functions are emitted only as specializations; the
                // worklist walks them.
                if f.variables(db).iter().any(is_iface_param(db)) {
                    continue;
                }
                process_body(
                    db,
                    f.scope_id(db),
                    f.statements(db),
                    &no_subs,
                    &mut by_canonical,
                    &mut instances,
                    &mut global_rewrites,
                );
            }
            Pou::FunctionBlock(fb) => {
                process_body(
                    db,
                    fb.scope_id(db),
                    fb.statements(db),
                    &no_subs,
                    &mut by_canonical,
                    &mut instances,
                    &mut global_rewrites,
                );
                for m in fb.methods(db) {
                    process_body(
                        db,
                        m.scope_id(db),
                        m.stmts(db),
                        &no_subs,
                        &mut by_canonical,
                        &mut instances,
                        &mut global_rewrites,
                    );
                }
            }
            Pou::Class(c) => {
                for m in c.methods(db) {
                    process_body(
                        db,
                        m.scope_id(db),
                        m.stmts(db),
                        &no_subs,
                        &mut by_canonical,
                        &mut instances,
                        &mut global_rewrites,
                    );
                }
            }
            _ => {}
        }
    }

    // Program bodies can also call interface-param functions.
    for (program, _) in all_programs {
        process_body(
            db,
            program.scope_id(db),
            program.statements(db),
            &no_subs,
            &mut by_canonical,
            &mut instances,
            &mut global_rewrites,
        );
    }

    // Worklist to fixpoint: `by_canonical` dedups, so this terminates even
    // for mutually forwarding functions.
    let mut i = 0;
    while i < instances.len() {
        let func = instances[i].func;
        let subs = instances[i].iface_subs.clone();
        let mut inst_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
        process_body(
            db,
            func.scope_id(db),
            func.statements(db),
            &subs,
            &mut by_canonical,
            &mut instances,
            &mut inst_rewrites,
        );
        instances[i].call_rewrites = inst_rewrites;
        i += 1;
    }

    (instances, global_rewrites)
}

/// Collect and process every call in one body under the active
/// substitution `subs`; rewrites are written to `out_rewrites`.
fn process_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    stmts: &[Stmt<'db>],
    subs: &FxHashMap<Ident, Pou<'db>>,
    by_canonical: &mut FxHashMap<(Ident, Vec<(Ident, Ident)>), Ident>,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
    // The POU whose instance `THIS` refers to in this body: the FB/Class itself
    // for its own body, or the owner for a method body. Used to resolve a `THIS`
    // interface argument (`f(dev := THIS)`) to its concrete type.
    let self_pou = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) if matches!(pou, Pou::FunctionBlock(_) | Pou::Class(_)) => Some(pou),
        _ => get_scope(db, scope).parent.and_then(|p| match get_scope(db, p).kind {
            ScopeKind::Pou(pou) if matches!(pou, Pou::FunctionBlock(_) | Pou::Class(_)) => Some(pou),
            _ => None,
        }),
    };

    let body = infer_body(db, scope);
    let mut calls = Vec::new();
    collect_calls(db, stmts, &mut calls);
    for fc in calls {
        process_call(
            db,
            fc,
            body,
            self_pou,
            subs,
            by_canonical,
            instances,
            out_rewrites,
        );
    }
}

/// Is `arg` a bare `THIS` reference (no trailing path)?
fn is_this_arg<'db>(db: &'db dyn WorkspaceDataBase, arg: Expr<'db>) -> bool {
    if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = arg.expr(db)
        && let VariableAccessKind::Symbolic(bp) = va.kind(db)
    {
        return bp.expr(db).is_none()
            && bp.invocation(db).map(|i| i.kind(db)) == Some(InvocationKind::This);
    }
    false
}

/// Recursively collect every `FuncCall` node in a statement list, including bare
/// statement-context calls (`bump(dev := w);`) and calls nested in control flow.
fn collect_calls<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    out: &mut Vec<FuncCall<'db>>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::FuncCall(fc) => {
                out.push(*fc);
                collect_calls_in_args(db, *fc, out);
            }
            StmtKind::Assignment { target, .. } => collect_calls_expr(db, *target, out),
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                collect_calls_expr(db, *condition, out);
                if let Some(s) = then {
                    collect_calls(db, s, out);
                }
                for (c, b) in else_if {
                    collect_calls_expr(db, *c, out);
                    collect_calls(db, b, out);
                }
                if let Some(s) = else_ {
                    collect_calls(db, s, out);
                }
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                collect_calls_expr(db, *condition, out);
                for (_, b) in cases {
                    collect_calls(db, b, out);
                }
                if let Some(s) = else_ {
                    collect_calls(db, s, out);
                }
            }
            StmtKind::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                collect_calls_expr(db, *start, out);
                collect_calls_expr(db, *end, out);
                if let Some(s) = step {
                    collect_calls_expr(db, *s, out);
                }
                collect_calls(db, body, out);
            }
            StmtKind::While { condition, body } | StmtKind::Repeat { condition, body } => {
                collect_calls_expr(db, *condition, out);
                collect_calls(db, body, out);
            }
            StmtKind::Raise { message } => collect_calls_expr(db, *message, out),
            _ => {}
        }
    }
}

/// Recursively collect `FuncCall` nodes reachable from an expression.
fn collect_calls_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    out: &mut Vec<FuncCall<'db>>,
) {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(fc)) => {
            out.push(*fc);
            collect_calls_in_args(db, *fc, out);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => {
            collect_calls_expr(db, *inner, out)
        }
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            collect_calls_expr(db, *left, out);
            collect_calls_expr(db, *right, out);
        }
        ExprKind::UnaryOperator { expr: inner, .. } => collect_calls_expr(db, *inner, out),
        _ => {}
    }
}

/// Calls can appear as call arguments (`f(x := g())`); recurse into them.
fn collect_calls_in_args<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    out: &mut Vec<FuncCall<'db>>,
) {
    for pa in fc.params(db) {
        match pa.kind(db) {
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                collect_calls_expr(db, value, out);
            }
            ParamAssignKind::FormalOutput { .. } => {}
        }
    }
}

fn process_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    body: &hir::hir_ty::body::BodyInferenceResult<'db>,
    self_pou: Option<Pou<'db>>,
    subs: &FxHashMap<Ident, Pou<'db>>,
    by_canonical: &mut FxHashMap<(Ident, Vec<(Ident, Ident)>), Ident>,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
    // The callee must be a plain function.
    let func = match fc.path(db).infer(db) {
        Type::Function(f) => f,
        Type::CallableType(CallableType::Function(f)) => f,
        _ => return,
    };

    // Does it take any interface parameter? (Design 1: only Input/InOut.)
    let has_iface_param = func.variables(db).iter().any(is_iface_param(db));
    if !has_iface_param {
        return;
    }

    // Bind each interface param to the concrete POU of its argument.
    let mut iface_subs: FxHashMap<Ident, Pou<'db>> = FxHashMap::default();
    for pa in fc.params(db) {
        let Some(param) = body.variable_of_param.get(&pa).copied() else {
            continue;
        };
        if !is_iface_param(db)(&param) {
            continue;
        }
        let arg = match pa.kind(db) {
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                value
            }
            ParamAssignKind::FormalOutput { .. } => continue,
        };
        // `THIS` resolves to the enclosing FB/Class; otherwise the argument's
        // concrete type, through the active substitution.
        let concrete = if is_this_arg(db, arg) {
            self_pou
        } else {
            let arg_ty = body
                .type_of_expr
                .get(&arg)
                .copied()
                .unwrap_or_else(|| arg.infer(db));
            resolve_concrete(db, arg_ty, subs)
        };
        if let Some(concrete) = concrete {
            iface_subs.insert(param.name(db), concrete);
        }
    }
    if iface_subs.is_empty() {
        return;
    }

    // Mangle: `drive` + `$<concrete>` per interface param, sorted by param name.
    let func_q = qualified_pou_ident(db, Type::Function(func));
    let mut sorted: Vec<(Ident, Pou<'db>)> = iface_subs.iter().map(|(k, v)| (*k, *v)).collect();
    sorted.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
    let key_concretes: Vec<(Ident, Ident)> = sorted
        .iter()
        .map(|(name, pou)| (*name, qualified_pou_ident(db, Type::new_pou(db, *pou))))
        .collect();
    let key = (func_q, key_concretes.clone());

    let mangled = match by_canonical.get(&key) {
        Some(m) => *m,
        None => {
            let parts: Vec<&str> = key_concretes.iter().map(|(_, q)| q.text(db).as_str()).collect();
            let m = super::monomorphize::mangle_generic_name(db, func_q, &parts);
            by_canonical.insert(key, m);
            instances.push(IfaceInstance {
                func,
                iface_subs: iface_subs.clone(),
                mangled_name: m,
                // Filled when this instance is processed by the worklist.
                call_rewrites: FxHashMap::default(),
            });
            m
        }
    };
    out_rewrites.insert(fc, mangled);
}

/// A param is an interface param iff it is `VAR_IN_OUT` and its (direct) type is
/// an interface. Nested interfaces are rejected by E0516, so a direct
/// `Type::Interface` is the only case here. (Spike scope: `VAR_IN_OUT` only —
/// the natural form for interfaces; `VAR_INPUT` copy-semantics is a follow-up.)
fn is_iface_param<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> impl Fn(&VariableDecl<'db>) -> bool + 'db {
    move |var: &VariableDecl<'db>| {
        matches!(var.kind(db), VariableKind::InOut)
            && matches!(var.spec(db).infer(db).normalize(db), Type::Interface(_))
    }
}

/// Resolve an argument's type to the concrete implementer it binds, honoring the
/// active substitution. The raw type of a bare variable access is
/// `Type::Variable((decl, _))` (before `normalize` peels it); if `decl` is an
/// interface param bound by `subs` — i.e. a *forwarded* interface param in a
/// specialized body — its concrete implementer is fixed there. Names are unique
/// within a scope and interface locals are forbidden (E0514), so matching the
/// substitution by the variable's name is unambiguous. Otherwise fall back to the
/// static concrete type (a concrete FB/Class arg, or `THIS` handled by the caller).
fn resolve_concrete<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    subs: &FxHashMap<Ident, Pou<'db>>,
) -> Option<Pou<'db>> {
    if let Type::Variable((var_decl, _)) = ty
        && let Some(pou) = subs.get(&var_decl.name(db))
    {
        return Some(*pou);
    }
    concrete_pou_of(db, ty)
}

/// The concrete FB/Class POU a value type denotes, or `None`.
fn concrete_pou_of<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Pou<'db>> {
    match ty.normalize(db).as_pou(db)? {
        p @ (Pou::FunctionBlock(_) | Pou::Class(_)) => Some(p),
        _ => None,
    }
}

