// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Phase B: parameter monomorphization. An interface only appears as a
//! direct `VAR_INPUT` / `VAR_IN_OUT` parameter, so at every call site the
//! concrete implementer is statically known: the callee is specialized per
//! binding (`drive$@Worker`, `Caller#Use$@Worker`) and the call rewritten.
//! An `ARRAY[*]` parameter is specialized the same way, per array type bound
//! to it (`Sum$[0..9]`), so its bounds are constants in each copy.

use db::WorkspaceDataBase;
use rustc_hash::{FxHashMap, FxHashSet};

use hir::hir_def::expressions::expression::{
    Expr, ExprKind, FuncCall, ParamAssignKind, PrimaryExpr, VariableAccessKind,
};
use hir::hir_def::expressions::invocation::InvocationKind;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::class::MethodDecl;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::function_block::FunctionBlock;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::variable::{VariableDecl, VariableKind};
use hir::hir_def::scope::ScopeId;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::oop::MethodRef;
use hir::hir_ty::ty::{CallableType, Type};

use super::naming::qualified_pou_ident;
use crate::types::MirType;

/// Call-site `FuncCall` → the mangled specialization it must call,
/// threaded into every body.
pub type IfaceCallRewrites<'db> = FxHashMap<FuncCall<'db>, Ident>;

/// The specializations requested so far, by mangled name, which says what
/// each is of and what it binds: one is emitted however many calls ask.
type CanonicalInstanceMap = FxHashSet<Ident>;

/// What gets specialized: a free FUNCTION, a METHOD, or the body of a
/// FUNCTION_BLOCK with an `ARRAY[*]` VAR_IN_OUT; they differ only in where
/// the copy is emitted and in the mangled-name base.
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
    /// The body of `block` emitted on the instance type `owner`: its own,
    /// or a base's its `SUPER()` reaches.
    Body {
        owner: Pou<'db>,
        block: FunctionBlock<'db>,
    },
}

impl<'db> IfaceTarget<'db> {
    /// The scope of the target's body, for the worklist walk.
    fn scope(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            IfaceTarget::Function(f) => f.scope_id(db),
            IfaceTarget::Method { method, .. } => method.scope_id(db),
            IfaceTarget::Body { block, .. } => block.scope_id(db),
        }
    }

    /// The symbol of the un-specialized callee, which the specialization
    /// extends. An overloaded callee starts from its own symbol: two
    /// overloads specialized alike are two bodies.
    fn base_symbol(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            IfaceTarget::Function(f) => super::naming::mir_function_symbol(db, *f),
            IfaceTarget::Method { owner, method } => {
                super::naming::method_copy_symbol(db, *owner, *method)
            }
            IfaceTarget::Body { owner, block } => {
                super::naming::body_symbol(db, *owner, Pou::FunctionBlock(*block))
            }
        }
    }
}

/// What a specialization binds to the parameters it is specialized on, by
/// declaration, not by name: a use of the parameter in another case is the
/// parameter, and another variable named like it is not.
#[derive(Clone, Debug, Default)]
pub struct ParamSubs<'db> {
    /// Each interface parameter and the concrete implementer bound to it.
    pub implementers: FxHashMap<VariableDecl<'db>, Pou<'db>>,
    /// Each `ARRAY[*]` parameter and the array type bound to it.
    pub shapes: FxHashMap<VariableDecl<'db>, MirType>,
}

impl<'db> ParamSubs<'db> {
    pub fn is_empty(&self) -> bool {
        self.implementers.is_empty() && self.shapes.is_empty()
    }

    /// What a base's body sees of this binding: the `ARRAY[*]` VAR_IN_OUTs
    /// it declares or inherits, those of the block that extends it left out.
    pub(crate) fn for_block(&self, db: &'db dyn WorkspaceDataBase, block: Pou<'db>) -> Self {
        let members = hir::hir_ty::oop::instance_members(db, block);
        ParamSubs {
            implementers: FxHashMap::default(),
            shapes: self
                .shapes
                .iter()
                .filter(|(var, _)| members.iter().any(|m| m.var == **var))
                .map(|(var, shape)| (*var, shape.clone()))
                .collect(),
        }
    }
}

/// The symbol of `base` specialized on `subs`: one fragment per specialized
/// param, sorted by name, `$@<implementer>` or `$[<bounds>]`.
pub(crate) fn specialized_symbol<'db>(
    db: &'db dyn WorkspaceDataBase,
    base: Ident,
    subs: &ParamSubs<'db>,
) -> Ident {
    let mut fragments: Vec<(Ident, String)> = subs
        .implementers
        .iter()
        .map(|(param, pou)| {
            let implementer = qualified_pou_ident(db, Type::new_pou(db, *pou));
            (
                param.name(db),
                super::naming::implementer_fragment(db, implementer),
            )
        })
        .chain(
            subs.shapes
                .iter()
                .map(|(param, shape)| (param.name(db), super::naming::shape_fragment(shape))),
        )
        .collect();
    fragments.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
    let parts: Vec<&str> = fragments.iter().map(|(_, f)| f.as_str()).collect();
    super::naming::mangle_generic_name(db, base, &parts)
}

/// Whether the instances of `block` hold an `ARRAY[*]` VAR_IN_OUT, its own or
/// a base's: its body is then emitted once per array type its calls bind.
pub(crate) fn has_conformand_in_out<'db>(db: &'db dyn WorkspaceDataBase, block: Pou<'db>) -> bool {
    hir::hir_ty::oop::instance_members(db, block)
        .iter()
        .any(|m| m.var.is_in_out(db) && m.var.conformand(db).is_some())
}

/// One specialization of a function or method on the concrete implementers
/// bound to its interface parameters.
pub struct IfaceInstance<'db> {
    pub target: IfaceTarget<'db>,
    /// What each specialized parameter is bound to.
    pub param_subs: ParamSubs<'db>,
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
    let mut by_canonical: CanonicalInstanceMap = FxHashSet::default();
    let mut instances: Vec<IfaceInstance<'db>> = Vec::new();
    // Rewrites for the generic bodies; specializations own theirs.
    let mut global_rewrites: FxHashMap<FuncCall<'db>, Ident> = FxHashMap::default();
    let mut owner_rewrites: OwnerRewrites<'db> = FxHashMap::default();
    let no_subs = ParamSubs::default();

    // --- Seed ---
    for (pou, _) in all_pous {
        // Walk every body of this POU — its own statements plus each method's —
        // for calls in BOTH expression and statement context.
        match pou {
            Pou::Function(f) => {
                // Functions with an interface or `ARRAY[*]` parameter are
                // emitted only as specializations; the worklist walks them.
                if f.variables(db).iter().any(is_specialized(db)) {
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
                    // Methods with an interface or `ARRAY[*]` parameter are
                    // emitted only as specializations; the worklist walks them.
                    .filter(|m| !m.variables(db).iter().any(is_specialized(db)))
                    .map(|m| m.scope_id(db))
                    .collect();
                // A body with an `ARRAY[*]` VAR_IN_OUT is emitted only as
                // specializations; the worklist walks them.
                if let Pou::FunctionBlock(fb) = pou
                    && !has_conformand_in_out(db, **pou)
                {
                    scopes.push(fb.scope_id(db));
                }
                scopes.extend(
                    copies
                        .bodies
                        .iter()
                        .filter(|b| !has_conformand_in_out(db, Pou::FunctionBlock(**b)))
                        .map(|b| b.scope_id(db)),
                );
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
        let subs = instances[i].param_subs.clone();
        let scope = target.scope(db);
        let self_pou = match target {
            IfaceTarget::Method { owner, .. } | IfaceTarget::Body { owner, .. } => Some(owner),
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
        // `SUPER()` runs the base's body in the same call, bound alike: the
        // base's copy for the arrays it sees.
        if let IfaceTarget::Body { owner, block } = target
            && scope.inference(db).first_super_body().is_some()
            && let Some(base @ Pou::FunctionBlock(base_block)) =
                hir::hir_ty::oop::explicit_bases(db, Pou::FunctionBlock(block)).extends
            && has_conformand_in_out(db, base)
        {
            instantiate(
                db,
                IfaceTarget::Body {
                    owner,
                    block: base_block,
                },
                subs.for_block(db, base),
                &mut by_canonical,
                &mut instances,
            );
        }
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
    subs: &ParamSubs<'db>,
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
    subs: &ParamSubs<'db>,
    by_canonical: &mut CanonicalInstanceMap,
    instances: &mut Vec<IfaceInstance<'db>>,
    out_rewrites: &mut FxHashMap<FuncCall<'db>, Ident>,
) {
    // The callee: a function or an instance method. A call through an
    // interface parameter runs the method of the implementer bound to it.
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
        Type::MethodDecl(MethodRef::Prototype(proto))
        | Type::CallableType(CallableType::MethodDecl(MethodRef::Prototype(proto))) => {
            match implementer_target(db, fc, proto, inference, subs) {
                Some(target) => target,
                None => return,
            }
        }
        // An instance's call runs its own type's body.
        Type::CallableType(CallableType::FunctionBlock(block)) => IfaceTarget::Body {
            owner: Pou::FunctionBlock(block),
            block,
        },
        _ => return,
    };

    // Does it take any interface or `ARRAY[*]` parameter?
    let callee_vars: Vec<VariableDecl<'db>> = match &target {
        IfaceTarget::Function(f) => f.variables(db).to_vec(),
        IfaceTarget::Method { method, .. } => method.variables(db).to_vec(),
        IfaceTarget::Body { owner, .. } => hir::hir_ty::oop::instance_members(db, *owner)
            .iter()
            .map(|m| m.var)
            .collect(),
    };
    if !callee_vars.iter().any(is_specialized(db)) {
        return;
    }

    // Bind each to what the call passes it.
    let mut bound = ParamSubs::default();
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
        let shape = match pa.kind(db) {
            // An `ARRAY[*]` output is written into the array it is bound to,
            // by `=>` or by position.
            kind if param.is_output(db) => match kind.value().variable_access(db) {
                Some(variable) if param.conformand(db).is_some() => output_shape(
                    db,
                    param,
                    inference.type_of_variable_access_adjusted(variable),
                    subs,
                ),
                _ => continue,
            },
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                if is_interface_param(db, &param) {
                    // `THIS` resolves to the enclosing FB/Class; otherwise the
                    // argument's concrete type, through the active substitution.
                    let concrete = if is_this_arg(db, value) {
                        self_pou
                    } else {
                        // Adjusted: `arr[i]` and `r^` bind the element and the target.
                        resolve_concrete(db, inference.type_of_expr_adjusted(value), subs)
                    };
                    if let Some(concrete) = concrete {
                        bound.implementers.insert(param, concrete);
                    }
                    continue;
                }
                if param.conformand(db).is_none() {
                    continue;
                }
                argument_shape(db, inference, param, value, subs)
            }
            ParamAssignKind::FormalOutput { .. } => continue,
        };
        if let Some(shape) = shape {
            bound.shapes.insert(param, shape);
        }
    }
    if bound.is_empty() {
        return;
    }
    let mangled = instantiate(db, target, bound, by_canonical, instances);
    out_rewrites.insert(fc, mangled);
}

/// The specialization of `target` on `bound`, requested once whatever the
/// number of calls: its symbol.
fn instantiate<'db>(
    db: &'db dyn WorkspaceDataBase,
    target: IfaceTarget<'db>,
    bound: ParamSubs<'db>,
    by_canonical: &mut CanonicalInstanceMap,
    instances: &mut Vec<IfaceInstance<'db>>,
) -> Ident {
    let mangled = specialized_symbol(db, target.base_symbol(db), &bound);
    if by_canonical.insert(mangled) {
        instances.push(IfaceInstance {
            target,
            param_subs: bound,
            mangled_name: mangled,
            // Filled when this instance is processed by the worklist.
            call_rewrites: FxHashMap::default(),
        });
    }
    mangled
}

/// The method a call through an interface parameter runs: the one the
/// implementer bound to the parameter answers to, as the instance type runs
/// it.
fn implementer_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    proto: hir::hir_def::pous::interface::MethodPrototype<'db>,
    inference: hir::hir_ty::body::ScopeInference<'db>,
    subs: &ParamSubs<'db>,
) -> Option<IfaceTarget<'db>> {
    use hir::HasName;
    use hir::hir_def::expressions::expression::PathExprKind;
    let PathExprKind::Field(field) = fc.path(db).expr(db)?.expr(db) else {
        return None;
    };
    let receiver =
        inference.variable_for_path_expr(field.path.flatten(db).first()?.get_expr(db))?;
    let owner = *subs.implementers.get(&receiver)?;
    let method =
        hir::hir_ty::oop::class_members(db, owner).implementation(&proto.get_name_ident(db))?;
    Some(IfaceTarget::Method { owner, method })
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

/// A parameter a callee is specialized on: an interface one, or an
/// `ARRAY[*]`.
pub(crate) fn is_specialized_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: &VariableDecl<'db>,
) -> bool {
    is_interface_param(db, var) || var.conformand(db).is_some()
}

fn is_specialized<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> impl Fn(&VariableDecl<'db>) -> bool + 'db {
    move |var: &VariableDecl<'db>| is_specialized_param(db, var)
}

/// An argument's concrete implementer under the active substitution: a
/// forwarded interface param is fixed by `subs`; otherwise the static
/// concrete type.
fn resolve_concrete<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    subs: &ParamSubs<'db>,
) -> Option<Pou<'db>> {
    if let Type::Variable((var_decl, _)) = ty
        && let Some(pou) = subs.implementers.get(&var_decl)
    {
        return Some(*pou);
    }
    concrete_pou_of(db, ty)
}

/// The array type a call binds to `param`, an `ARRAY[*]`: the argument's,
/// or the row of an array it names (`m[i]`), through the active
/// substitution. For one of any type only the bounds count, so one copy of
/// the FUNCTION serves every element type.
pub(crate) fn argument_shape<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: hir::hir_ty::body::ScopeInference<'db>,
    param: VariableDecl<'db>,
    value: Expr<'db>,
    subs: &ParamSubs<'db>,
) -> Option<MirType> {
    let row = match value.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => match access.kind(db) {
            VariableAccessKind::Symbolic(begin) => begin
                .expr(db)
                .and_then(|path| inference.indexed_array(path))
                .filter(|indexed| indexed.is_partial(db)),
            VariableAccessKind::Direct(_) => None,
        },
        _ => None,
    };
    let shape = match row {
        Some(row) => {
            let whole = match row.array {
                hir::hir_ty::body::IndexedType::Array(array) => {
                    resolve_shape(db, Type::Array(array), subs)?
                }
                hir::hir_ty::body::IndexedType::Conformand(conformand) => subs
                    .shapes
                    .iter()
                    .find(|(var, _)| var.conformand(db) == Some(conformand))
                    .map(|(_, shape)| shape.clone())?,
            };
            row_of(&whole, row.through)?
        }
        None => resolve_shape(db, inference.type_of_expr_adjusted(value), subs)?,
    };
    Some(for_param(db, param, shape))
}

/// The array type an `ARRAY[*]` output is written into.
pub(crate) fn output_shape<'db>(
    db: &'db dyn WorkspaceDataBase,
    param: VariableDecl<'db>,
    destination: Type<'db>,
    subs: &ParamSubs<'db>,
) -> Option<MirType> {
    resolve_shape(db, destination, subs).map(|shape| for_param(db, param, shape))
}

/// What a copy for `param` depends on of the array `shape`: all of it, or
/// only the bounds for an `ARRAY[*]` of any type.
fn for_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    param: VariableDecl<'db>,
    shape: MirType,
) -> MirType {
    match (param.conformand(db).and_then(|c| c.of_type(db)), shape) {
        (None, MirType::Array(array)) => MirType::Array(crate::types::MirArrayType {
            element_type: Box::new(MirType::Void),
            dimensions: array.dimensions,
            total_elements: array.total_elements,
            element_size: 0,
            size: 0,
            align: 1,
        }),
        (_, shape) => shape,
    }
}

/// The rows of `whole` after its first `through` dimensions.
fn row_of(whole: &MirType, through: usize) -> Option<MirType> {
    match whole {
        MirType::Array(array) => array.row(through).map(MirType::Array),
        _ => None,
    }
}

/// The array type an `ARRAY[*]` argument binds: a forwarded parameter's, as
/// the active substitution has it, or the argument's own.
pub(crate) fn resolve_shape<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    subs: &ParamSubs<'db>,
) -> Option<MirType> {
    if let Type::Variable((var_decl, _)) = ty
        && let Some(shape) = subs.shapes.get(&var_decl)
    {
        return Some(shape.clone());
    }
    match super::lower_type::lower_type(db, ty) {
        Ok(shape @ MirType::Array(_)) => Some(shape),
        _ => None,
    }
}

/// The concrete FB/Class POU a value type denotes, or `None`.
fn concrete_pou_of<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Pou<'db>> {
    match ty.normalize(db).as_pou(db)? {
        p @ (Pou::FunctionBlock(_) | Pou::Class(_)) => Some(p),
        _ => None,
    }
}
