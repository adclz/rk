//! A variadic parameter collects however many arguments its call site
//! supplies, so `sum_all` is specialized per arity called (`sum_all$3`) and
//! the call rewritten to it; inside, the pack is that many ordinary
//! parameters and `...args+` folds over them. No worklist: a pack can only
//! be consumed by a fold, so one pass over every body finds every arity.
//! The arity is HIR's `ParamBinding::Values` on the variadic parameter.

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use hir::hir_def::expressions::expression::FuncCall;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::function::Function;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::scope::ScopeId;
use hir::hir_ty::body::{ParamBinding, infer_body};
use hir::hir_ty::ty::CallableType;

use super::naming::{mangle_generic_name, mir_function_symbol};

/// Call site → the arity its callee was specialized at; the call site
/// appends `$N` to the symbol it resolved, composing with overload
/// mangling.
pub type ArityCallRewrites<'db> = FxHashMap<FuncCall<'db>, usize>;

/// One specialization of a variadic FUNCTION at a concrete argument count.
pub struct ArityInstance<'db> {
    pub func: Function<'db>,
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

/// Walk every body; for each call to a variadic function, record the arity
/// specialization it needs and the call-site -> mangled-name rewrite.
pub fn collect_arity_instantiations<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
) -> (Vec<ArityInstance<'db>>, ArityCallRewrites<'db>) {
    // (function symbol, arity) -> mangled name, so one specialization is
    // emitted however many call sites ask for it.
    let mut by_canonical: FxHashMap<(Ident, usize), Ident> = FxHashMap::default();
    let mut instances: Vec<ArityInstance<'db>> = Vec::new();
    let mut rewrites: ArityCallRewrites<'db> = FxHashMap::default();

    // Every body is walked, a variadic function's own included: its calls
    // have their own fixed arity.
    for (pou, _) in all_pous {
        match pou {
            Pou::Function(f) => process_body(
                db,
                f.scope_id(db),
                &mut by_canonical,
                &mut instances,
                &mut rewrites,
            ),
            Pou::FunctionBlock(fb) => {
                process_body(
                    db,
                    fb.scope_id(db),
                    &mut by_canonical,
                    &mut instances,
                    &mut rewrites,
                );
                for m in fb.methods(db) {
                    process_body(
                        db,
                        m.scope_id(db),
                        &mut by_canonical,
                        &mut instances,
                        &mut rewrites,
                    );
                }
            }
            Pou::Class(c) => {
                for m in c.methods(db) {
                    process_body(
                        db,
                        m.scope_id(db),
                        &mut by_canonical,
                        &mut instances,
                        &mut rewrites,
                    );
                }
            }
            _ => {}
        }
    }
    for (prog, _) in all_programs {
        process_body(
            db,
            prog.scope_id(db),
            &mut by_canonical,
            &mut instances,
            &mut rewrites,
        );
    }

    (instances, rewrites)
}

fn process_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    by_canonical: &mut FxHashMap<(Ident, usize), Ident>,
    instances: &mut Vec<ArityInstance<'db>>,
    rewrites: &mut ArityCallRewrites<'db>,
) {
    let body = infer_body(db, scope);
    // Every call resolution recorded, instead of a second walk over the tree.
    for fc in body.calls.clone() {
        let Some(record) = body.resolved_calls.get(&fc) else {
            continue;
        };
        let CallableType::Function(f) = record.callable else {
            continue;
        };
        // The pack's own binding is the argument count: resolution put every
        // value it collected there, in call order.
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

        let base = mir_function_symbol(db, f);
        by_canonical.entry((base, arity)).or_insert_with(|| {
            let name = mangle_generic_name(db, base, &[&arity.to_string()]);
            instances.push(ArityInstance {
                func: f,
                arity,
                mangled_name: name,
            });
            name
        });
        rewrites.insert(fc, arity);
    }
}
