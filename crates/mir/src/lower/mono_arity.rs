// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A variadic parameter collects however many arguments its call site
//! supplies, so `sum_all` is specialized per arity called (`sum_all$3`), and
//! a variadic METHOD per arity on the instance type a call runs it on
//! (`Acc#Sum$3`); the call site appends the arity to the symbol it resolved.
//! Inside, the pack is that many ordinary parameters and `...args+` folds
//! over them. No worklist: a pack can only be consumed by a fold, so one pass
//! over every body and every local initializer finds every arity.
//! The arity is HIR's `ParamBinding::Values` on the variadic parameter.

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use hir::HasName;
use hir::hir_def::expressions::expression::FuncCall;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::class::MethodDecl;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::scope::ScopeId;
use hir::hir_ty::body::{ParamBinding, ScopeInference};
use hir::hir_ty::oop::MethodRef;
use hir::hir_ty::ty::CallableType;

use super::mono_iface::IfaceTarget;
use super::naming::{mangle_generic_name, method_copy_symbol, mir_function_symbol};

/// Call site → the arity its callee was specialized at; the call site
/// appends `$N` to the symbol it resolved, composing with overload
/// mangling.
pub type ArityCallRewrites<'db> = FxHashMap<FuncCall<'db>, usize>;

/// What a specialization is of: a FUNCTION, or a METHOD as one instance type
/// runs it.
#[derive(Clone, Copy)]
pub enum ArityTarget<'db> {
    Function(Function<'db>),
    Method {
        owner: Pou<'db>,
        method: MethodDecl<'db>,
    },
}

/// One specialization of a variadic FUNCTION or METHOD at a concrete
/// argument count.
pub struct ArityInstance<'db> {
    pub target: ArityTarget<'db>,
    /// How many arguments the pack collects in this specialization.
    pub arity: usize,
    pub mangled_name: Ident,
}

/// The variadic `VAR_INPUT` of a function, if it declares one.
pub fn variadic_param<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<hir::hir_def::pous::variable::VariableDecl<'db>> {
    func.variables(db).iter().find(|v| v.variadic(db)).copied()
}

/// Walk every body and local initializer; for each call to a variadic
/// function or method, record the arity specialization it needs and the
/// call-site -> mangled-name rewrite.
pub fn collect_arity_instantiations<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
) -> (Vec<ArityInstance<'db>>, ArityCallRewrites<'db>) {
    // (function or method copy symbol, arity) -> mangled name, so one
    // specialization is emitted however many call sites ask for it.
    let mut by_canonical: FxHashMap<(Ident, usize), Ident> = FxHashMap::default();
    let mut instances: Vec<ArityInstance<'db>> = Vec::new();
    let mut rewrites: ArityCallRewrites<'db> = FxHashMap::default();
    let mut process = |scope: ScopeId<'db>, self_pou: Option<Pou<'db>>| {
        process_body(
            db,
            scope,
            self_pou,
            &mut by_canonical,
            &mut instances,
            &mut rewrites,
        )
    };

    // Every body is walked, a variadic function's own included: its calls
    // have their own fixed arity. An FB or CLASS walks every body it is
    // emitted with, `THIS` being the block, as the interface pass does.
    for (pou, _) in all_pous {
        match pou {
            Pou::Function(f) => process(f.scope_id(db), None),
            Pou::FunctionBlock(_) | Pou::Class(_) => {
                let copies = super::lower_func::instance_copies(db, **pou);
                for m in &copies.methods {
                    process(m.scope_id(db), Some(**pou));
                }
                if let Pou::FunctionBlock(fb) = pou {
                    process(fb.scope_id(db), Some(**pou));
                }
                for body in &copies.bodies {
                    process(body.scope_id(db), Some(**pou));
                }
            }
            _ => {}
        }
    }
    for (prog, _) in all_programs {
        process(prog.scope_id(db), None);
    }

    (instances, rewrites)
}

fn process_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    self_pou: Option<Pou<'db>>,
    by_canonical: &mut FxHashMap<(Ident, usize), Ident>,
    instances: &mut Vec<ArityInstance<'db>>,
    rewrites: &mut ArityCallRewrites<'db>,
) {
    // Every call resolution recorded, in the statements and the local
    // initializers (`x : INT := sum_all(1, 2, 3)` reached codegen with no
    // `sum_all$3`), instead of a second walk over the tree.
    let inference = scope.inference(db);
    for fc in inference.calls() {
        let Some(record) = inference.resolved_call(fc) else {
            continue;
        };
        // The pack's own binding is the argument count: resolution put
        // every value it collected there, in call order.
        let Some(arity) = record.params.iter().find_map(|(var, binding)| {
            if !var.variadic(db) {
                return None;
            }
            match binding {
                ParamBinding::Values(vs) => Some(vs.len()),
                // An empty pack is E0813; nothing to specialize.
                _ => None,
            }
        }) else {
            continue;
        };

        // A method runs as the instance type's copy, the one the call
        // site names.
        let targets = match record.callable {
            CallableType::Function(f) => vec![ArityTarget::Function(f)],
            CallableType::MethodDecl(MethodRef::Declared(md)) => {
                match super::mono_iface::method_target(db, fc, md, inference, self_pou) {
                    Some(IfaceTarget::Method { owner, method }) => {
                        vec![ArityTarget::Method { owner, method }]
                    }
                    _ => continue,
                }
            }
            CallableType::MethodDecl(MethodRef::Prototype(proto)) => {
                implementer_targets(db, fc, inference, proto.get_name_ident(db))
            }
            _ => continue,
        };
        for target in targets {
            let base = match target {
                ArityTarget::Function(f) => mir_function_symbol(db, f),
                ArityTarget::Method { owner, method } => method_copy_symbol(db, owner, method),
            };
            by_canonical.entry((base, arity)).or_insert_with(|| {
                let name = mangle_generic_name(db, base, &[&arity.to_string()]);
                instances.push(ArityInstance {
                    target,
                    arity,
                    mangled_name: name,
                });
                name
            });
        }
        rewrites.insert(fc, arity);
    }
}

/// The copies a call through an interface may run: the method each
/// implementer of the receiver's interface answers to, an implementer
/// deriving from another included. Which one runs is known only where the
/// interface parameter is bound, so each gets its copy for this count.
fn implementer_targets<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: FuncCall<'db>,
    inference: ScopeInference<'db>,
    name: Ident,
) -> Vec<ArityTarget<'db>> {
    use hir::hir_def::expressions::expression::PathExprKind;
    let Some(PathExprKind::Field(field)) = fc.path(db).expr(db).map(|pe| pe.expr(db)) else {
        return Vec::new();
    };
    let Some(interface @ Pou::Interface(_)) = inference
        .type_of_path_expr(field.path)
        .normalize(db)
        .as_pou(db)
    else {
        return Vec::new();
    };
    hir::hir_ty::oop::descendants(db, interface)
        .into_iter()
        .filter(|owner| matches!(owner, Pou::FunctionBlock(_) | Pou::Class(_)))
        .filter_map(|owner| {
            hir::hir_ty::oop::class_members(db, owner)
                .implementation(&name)
                .map(|method| ArityTarget::Method { owner, method })
        })
        .collect()
}
