// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Phase B: interface-parameter monomorphization. An interface only appears
//! as a direct `VAR_INPUT` / `VAR_IN_OUT` parameter, so at every call site
//! the concrete implementer is statically known: the callee is specialized
//! per binding (`drive$@Worker`, `Caller#Use$@Worker`) and the call rewritten.

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use hir::hir_def::expressions::expression::{
    Expr, ExprKind, FuncCall, ParamAssignKind, PrimaryExpr, VariableAccessKind,
};
use hir::hir_def::expressions::invocation::InvocationKind;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::class::MethodDecl;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::variable::{VariableDecl, VariableKind};
use hir::hir_def::scope::ScopeId;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::oop::MethodRef;
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
        /// The instance type (FB or Class) the call runs on, whose copy of
        /// the method is specialized: for an inherited method the inheritor,
        /// where `THIS` is the inheritor, not the declaring base.
        owner: Pou<'db>,
        method: MethodDecl<'db>,
    },
}

impl<'db> IfaceTarget<'db> {
    /// The scope of the target's body, for the worklist walk.
    fn scope(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            IfaceTarget::Function(f) => f.scope_id(db),
            IfaceTarget::Method { method, .. } => method.scope_id(db),
        }
    }
}

/// Each interface parameter of a specialization, by its declaration, and the
/// concrete implementer bound to it. By declaration, not by name: a use of
/// the parameter in another case is the parameter, and another variable
/// named like it is not.
pub type IfaceSubs<'db> = FxHashMap<VariableDecl<'db>, Pou<'db>>;

/// One specialization of a function or method on the concrete implementers
/// bound to its interface parameters.
pub struct IfaceInstance<'db> {
    pub target: IfaceTarget<'db>,
    /// interface param -> concrete implementer POU
    pub iface_subs: IfaceSubs<'db>,
    pub mangled_name: Ident,
    /// Rewrites for calls inside this specialization's body: a forwarded
    /// interface param resolves to a different transitive specialization per
    /// binding.
    pub call_rewrites: FxHashMap<FuncCall<'db>, Ident>,
}

/// The call rewrites of the bodies emitted on each FB or CLASS: an
/// inherited method or a base's body is one body emitted on several
/// instance types, and `THIS` in it, passed or called, is each of them.
pub type OwnerRewrites<'db> = FxHashMap<Pou<'db>, IfaceCallRewrites<'db>>;

/// Walk every POU body; for each call to a function with interface
/// parameters, record the specialization it needs and the call rewrite.
/// Seed from the generically emitted bodies, then process each
/// specialization with its own substitution active, to fixpoint. Returns
/// the specializations, the rewrites of FUNCTION and PROGRAM bodies, and
/// those of the bodies each FB or CLASS is emitted with.
pub fn collect_iface_instantiations<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
) -> (
    Vec<IfaceInstance<'db>>,
    FxHashMap<FuncCall<'db>, Ident>,
    OwnerRewrites<'db>,
) {
    let mut by_canonical: CanonicalInstanceMap = FxHashMap::default();
    let mut instances: Vec<IfaceInstance<'db>> = Vec::new();
    // Rewrites for the generic bodies; specializations own theirs.
    let mut global_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
    let mut owner_rewrites: OwnerRewrites<'db> = FxHashMap::default();
    let no_subs: IfaceSubs<'db> = FxHashMap::default();

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
                    None,
                    &no_subs,
                    &mut by_canonical,
                    &mut instances,
                    &mut global_rewrites,
                );
            }
            // Every body the block is emitted with, `THIS` being the block:
            // its methods, the ones it inherits, the base methods and bodies
            // its SUPER calls reach, and its own body.
            Pou::FunctionBlock(_) | Pou::Class(_) => {
                let copies = super::lower_func::instance_copies(db, **pou);
                let mut scopes: Vec<ScopeId<'db>> = copies
                    .methods
                    .iter()
                    // Interface-param methods are emitted only as
                    // specializations; the worklist walks them.
                    .filter(|m| !m.variables(db).iter().any(is_iface_param(db)))
                    .map(|m| m.scope_id(db))
                    .collect();
                if let Pou::FunctionBlock(fb) = pou {
                    scopes.push(fb.scope_id(db));
                }
                scopes.extend(copies.bodies.iter().map(|b| b.scope_id(db)));
                let rewrites = owner_rewrites.entry(**pou).or_default();
                for scope in scopes {
                    process_body(
                        db,
                        scope,
                        Some(**pou),
                        &no_subs,
                        &mut by_canonical,
                        &mut instances,
                        rewrites,
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
        let scope = target.scope(db);
        let self_pou = match target {
            IfaceTarget::Method { owner, .. } => Some(owner),
            IfaceTarget::Function(_) => None,
        };
        let mut inst_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
        process_body(
            db,
            scope,
            self_pou,
            &subs,
            &mut by_canonical,
            &mut instances,
            &mut inst_rewrites,
        );
        instances[i].call_rewrites = inst_rewrites;
        i += 1;
    }

    (instances, global_rewrites, owner_rewrites)
}

/// Collect and process every call in one body under the active
/// substitution `subs`; rewrites are written to `out_rewrites`.
#[allow(clippy::too_many_arguments)]
fn process_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    // The POU whose instance `THIS` refers to in this body, supplied by the
    // caller.
    self_pou: Option<Pou<'db>>,
    subs: &IfaceSubs<'db>,
    by_canonical: &mut CanonicalInstanceMap,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
    // Every call resolution recorded, in the statements and the local
    // initializers (`x : INT := ident(p)` reached codegen with no
    // `ident$@Pump`), instead of a second walk over the tree.
    let inference = scope.inference(db);
    for fc in inference.calls() {
        process_call(
            db,
            fc,
            inference,
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

#[allow(clippy::too_many_arguments)]
fn process_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    inference: hir::hir_ty::body::ScopeInference<'db>,
    self_pou: Option<Pou<'db>>,
    subs: &IfaceSubs<'db>,
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
            match method_target(db, fc, md, inference, self_pou) {
                Some(target) => target,
                None => return,
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
    let mut iface_subs: IfaceSubs<'db> = FxHashMap::default();
    for pa in fc.params(db) {
        let Some(param) = inference.variable_for_param(*pa) else {
            continue;
        };
        // The parameter of the method that runs: HIR bound the argument to the
        // method it checked the call against, and an override running in its
        // place declares its own, of the same name.
        let param = callee_vars
            .iter()
            .find(|v| v.name(db) == param.name(db))
            .copied()
            .unwrap_or(param);
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
            // Adjusted: `arr[i]` and `r^` bind the element and the target.
            resolve_concrete(db, inference.type_of_expr_adjusted(arg), subs)
        };
        if let Some(concrete) = concrete {
            iface_subs.insert(param, concrete);
        }
    }
    if iface_subs.is_empty() {
        return;
    }

    // The un-specialized callee's symbol, then `$@<concrete>` per interface
    // param, sorted by name. An overloaded callee starts from its own
    // symbol: two overloads specialized at one implementer are two bodies.
    let base = match &target {
        IfaceTarget::Function(f) => super::naming::mir_function_symbol(db, *f),
        IfaceTarget::Method { owner, method } => {
            super::naming::method_copy_symbol(db, *owner, *method)
        }
    };
    let mut sorted: Vec<(Ident, Pou<'db>)> =
        iface_subs.iter().map(|(k, v)| (k.name(db), *v)).collect();
    sorted.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
    let key_concretes: Vec<(Ident, Ident)> = sorted
        .iter()
        .map(|(name, pou)| (*name, qualified_pou_ident(db, Type::new_pou(db, *pou))))
        .collect();
    let key = (base, key_concretes.clone());

    let mangled = match by_canonical.get(&key) {
        Some(m) => *m,
        None => {
            let parts: Vec<String> = key_concretes
                .iter()
                .map(|(_, q)| super::naming::implementer_fragment(db, *q))
                .collect();
            let parts: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
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

/// The method a call runs, and the instance type whose copy of it runs:
/// `THIS.m()` and a bare `m()` the one this body's instance answers to,
/// `SUPER.m()` the base's copy on this instance, `inst.m()` and
/// `THIS.inner.m()` the method of the member's type, after indexing and
/// dereferencing.
pub(crate) fn method_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    resolved: MethodDecl<'db>,
    inference: hir::hir_ty::body::ScopeInference<'db>,
    self_pou: Option<Pou<'db>>,
) -> Option<IfaceTarget<'db>> {
    use hir::hir_def::expressions::expression::PathExprKind;
    let path = fc.path(db);
    let member = match path.expr(db).map(|pe| pe.expr(db)) {
        Some(PathExprKind::Field(fe)) => Some(fe.path),
        _ => None,
    };
    let invocation = path.invocation(db).map(|i| i.kind(db));
    match (member, invocation) {
        // `inst.m()`, `THIS.inner.m()`: HIR resolved the member type's method.
        (Some(receiver), _) => {
            let owner = concrete_pou_of(db, inference.type_of_path_expr_adjusted(receiver))?;
            Some(IfaceTarget::Method {
                owner,
                method: resolved,
            })
        }
        // `SUPER.m()`: the base's method, on this instance.
        (None, Some(InvocationKind::Super)) => Some(IfaceTarget::Method {
            owner: self_pou?,
            method: resolved,
        }),
        // `THIS.m()` and a bare `m()`: the method this instance answers to,
        // an override where the body is inherited code.
        _ => {
            let owner = self_pou?;
            let method = hir::hir_ty::oop::class_members(db, owner)
                .implementation(&resolved.name(db))
                .unwrap_or(resolved);
            Some(IfaceTarget::Method { owner, method })
        }
    }
}

/// A `VAR_INPUT` / `VAR_IN_OUT` param whose direct type is an interface
/// (nested interfaces are E1123). Both kinds pass the address: an
/// interface value is a reference, and rebinding is E1124.
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

/// An argument's concrete implementer under the active substitution: a
/// forwarded interface param is fixed by `subs`; otherwise the static
/// concrete type.
fn resolve_concrete<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    subs: &IfaceSubs<'db>,
) -> Option<Pou<'db>> {
    if let Type::Variable((var_decl, _)) = ty
        && let Some(pou) = subs.get(&var_decl)
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
