//! Phase B: interface-parameter monomorphization. An interface only appears
//! as a direct `VAR_INPUT` / `VAR_IN_OUT` parameter, so at every call site
//! the concrete implementer is statically known: the callee is specialized
//! per binding (`drive$Worker`, `Caller#Use$Worker`) and the call rewritten.

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use hir::hir_def::expressions::expression::{
    Expr, ExprKind, FuncCall, ParamAssignKind, PrimaryExpr, VariableAccessKind,
};
use hir::hir_def::expressions::invocation::InvocationKind;
use hir::hir_def::expressions::statement::{Stmt, StmtKind};
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::class::MethodDecl;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::variable::{VariableDecl, VariableKind};
use hir::hir_def::scope::{ScopeId, ScopeKind};
use hir::hir_def::semantic_index::get_scope;
use hir::hir_ty::body::infer_body;
use hir::hir_ty::head::inheritance::MethodRef;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::ty::{CallableType, Type};

use super::naming::qualified_pou_ident;

/// Call-site `FuncCall` → the mangled specialization it must call,
/// threaded into every body.
pub type IfaceCallRewrites<'db> = FxHashMap<FuncCall<'db>, Ident>;

/// Canonical identity of one specialization: `(qualified func name,
/// sorted [(interface param name, qualified concrete implementer)])`.
type CanonicalKey = (Ident, Vec<(Ident, Ident)>);

/// Specialization canonical key → mangled name.
type CanonicalInstanceMap = FxHashMap<CanonicalKey, Ident>;

/// What gets specialized: a free FUNCTION or a METHOD; they differ only in
/// where the copy is emitted and in the mangled-name base.
#[derive(Clone, Copy)]
pub enum IfaceTarget<'db> {
    Function(Function<'db>),
    Method {
        /// The DECLARING owner (FB or Class) — for an inherited method this is
        /// the base, matching the `Owner#method` symbol convention.
        owner: Pou<'db>,
        method: MethodDecl<'db>,
    },
}

impl<'db> IfaceTarget<'db> {
    /// The scope + statements of the target's body, for the worklist walk.
    fn body(&self, db: &'db dyn WorkspaceDataBase) -> (ScopeId<'db>, &'db [Stmt<'db>]) {
        match self {
            IfaceTarget::Function(f) => (f.scope_id(db), f.statements(db)),
            IfaceTarget::Method { method, .. } => (method.scope_id(db), method.stmts(db)),
        }
    }
}

/// One specialization of a function or method on the concrete implementers
/// bound to its interface parameters.
pub struct IfaceInstance<'db> {
    pub target: IfaceTarget<'db>,
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
    let mut by_canonical: CanonicalInstanceMap = FxHashMap::default();
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
                    None,
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
                    Some(**pou),
                    &no_subs,
                    &mut by_canonical,
                    &mut instances,
                    &mut global_rewrites,
                );
                for m in fb.methods(db) {
                    // Same for interface-param methods.
                    if m.variables(db).iter().any(is_iface_param(db)) {
                        continue;
                    }
                    process_body(
                        db,
                        m.scope_id(db),
                        m.stmts(db),
                        Some(**pou),
                        &no_subs,
                        &mut by_canonical,
                        &mut instances,
                        &mut global_rewrites,
                    );
                }
            }
            Pou::Class(c) => {
                for m in c.methods(db) {
                    if m.variables(db).iter().any(is_iface_param(db)) {
                        continue;
                    }
                    process_body(
                        db,
                        m.scope_id(db),
                        m.stmts(db),
                        Some(**pou),
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
            None,
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
        let target = instances[i].target;
        let subs = instances[i].iface_subs.clone();
        let (scope, stmts) = target.body(db);
        let self_pou = match target {
            IfaceTarget::Method { owner, .. } => Some(owner),
            IfaceTarget::Function(_) => None,
        };
        let mut inst_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
        process_body(
            db,
            scope,
            stmts,
            self_pou,
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
#[allow(clippy::too_many_arguments)]
fn process_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    stmts: &[Stmt<'db>],
    // The POU whose instance `THIS` refers to in this body, supplied by the
    // caller.
    self_pou: Option<Pou<'db>>,
    subs: &FxHashMap<Ident, Pou<'db>>,
    by_canonical: &mut CanonicalInstanceMap,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
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

#[allow(clippy::too_many_arguments)]
fn process_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    body: &hir::hir_ty::body::BodyInferenceResult<'db>,
    self_pou: Option<Pou<'db>>,
    subs: &FxHashMap<Ident, Pou<'db>>,
    by_canonical: &mut CanonicalInstanceMap,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
    // The callee: a function or an instance method. Interface-method
    // prototypes are the receiver-dispatch side, not a specializable callee.
    let target = match fc.path(db).infer(db) {
        Type::Function(f) => IfaceTarget::Function(f),
        Type::CallableType(CallableType::Function(f)) => IfaceTarget::Function(f),
        Type::MethodDecl(MethodRef::Declared(md))
        | Type::CallableType(CallableType::MethodDecl(MethodRef::Declared(md))) => {
            // The declaring owner, the POU the `Owner#method` symbol is registered
            // under.
            let parent = match get_scope(db, md.scope_id(db)).parent {
                Some(p) => p,
                None => return,
            };
            match get_scope(db, parent).kind {
                ScopeKind::Pou(owner @ (Pou::FunctionBlock(_) | Pou::Class(_))) => {
                    IfaceTarget::Method { owner, method: md }
                }
                _ => return,
            }
        }
        _ => return,
    };

    // Does it take any interface parameter? (Design 1: only Input/InOut.)
    let callee_vars: &[VariableDecl<'db>] = match &target {
        IfaceTarget::Function(f) => f.variables(db),
        IfaceTarget::Method { method, .. } => method.variables(db),
    };
    if !callee_vars.iter().any(is_iface_param(db)) {
        return;
    }

    // Bind each interface param to the concrete POU of its argument.
    let mut iface_subs: FxHashMap<Ident, Pou<'db>> = FxHashMap::default();
    for pa in fc.params(db) {
        let Some(param) = body.variable_of_param.get(pa).copied() else {
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

    // The un-specialized callee's symbol, then `$<concrete>` per interface
    // param, sorted by name.
    let base = match &target {
        IfaceTarget::Function(f) => qualified_pou_ident(db, Type::Function(*f)),
        IfaceTarget::Method { owner, method } => {
            let owner_q = qualified_pou_ident(db, Type::new_pou(db, *owner));
            Ident::new(
                db,
                compact_str::CompactString::from(format!(
                    "{}#{}",
                    owner_q.text(db),
                    method.name(db).text(db)
                )),
            )
        }
    };
    let mut sorted: Vec<(Ident, Pou<'db>)> = iface_subs.iter().map(|(k, v)| (*k, *v)).collect();
    sorted.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
    let key_concretes: Vec<(Ident, Ident)> = sorted
        .iter()
        .map(|(name, pou)| (*name, qualified_pou_ident(db, Type::new_pou(db, *pou))))
        .collect();
    let key = (base, key_concretes.clone());

    let mangled = match by_canonical.get(&key) {
        Some(m) => *m,
        None => {
            let parts: Vec<&str> = key_concretes
                .iter()
                .map(|(_, q)| q.text(db).as_str())
                .collect();
            let m = super::naming::mangle_generic_name(db, base, &parts);
            by_canonical.insert(key, m);
            instances.push(IfaceInstance {
                target,
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

/// A param is an interface param iff it is a `VAR_INPUT` / `VAR_IN_OUT` param and
/// its (direct) type is an interface. Nested interfaces are rejected by E0516, so
/// a direct `Type::Interface` is the only case here. Both kinds monomorphize
/// identically: an interface value is a *reference*, so `VAR_INPUT` passes the
/// address (a copy of the reference) and `VAR_IN_OUT` passes the reference — the
/// callee receives a pointer to the concrete instance either way, and we have
/// banned rebinding (E0517), which is the only behavioural difference between
/// them. (Matches other toolchains: the `INTERFACE` keyword forces address-passing for
/// both.)
pub(crate) fn is_interface_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: &VariableDecl<'db>,
) -> bool {
    matches!(var.kind(db), VariableKind::InOut | VariableKind::Input)
        && matches!(var.spec(db).infer(db).normalize(db), Type::Interface(_))
}

fn is_iface_param<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> impl Fn(&VariableDecl<'db>) -> bool + 'db {
    move |var: &VariableDecl<'db>| is_interface_param(db, var)
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
