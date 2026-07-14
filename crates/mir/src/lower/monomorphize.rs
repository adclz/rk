//! Monomorphization of ANY_* functions.
//!
//! ANY_* functions are polymorphic - they accept parameters typed as AnyInt, AnyReal, etc.
//! This module handles instantiating concrete copies for each call site.
//!
//! The approach:
//! 1. Detect ANY_* functions during initial lowering (deferred, not lowered yet)
//! 2. Walk all lowered function bodies to discover which concrete types are used at call sites
//! 3. For each (any_func, concrete_type) pair:
//!    - Extern: create a `MirExternFunction` with suffixed import name
//!    - Local: clone the template body with type substitution
//! 4. Rewrite call sites to reference the monomorphized copies

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::spec::ElementarySpec, extern_decl::ExternDecl, interned::identifier::Ident,
        pous::function::Function,
    },
    hir_ty::{infer::Infer, ty::Type},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    MirModule,
    expr::{MirArgKind, MirCall, MirConstant, MirExpr},
    function::{
        MirExternFunction, MirFunction, MirLinkage, MirLocal, MirLocalKind, MirParam, MirParamKind,
        MirVariableStorage,
    },
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir, lower_type},
    memory::MirMemoryLayout,
    stmt::MirStmt,
    types::{MirElementary, MirType},
};

/// Info about an ANY_* function pending monomorphization.
///
/// The wasm/extern classification is intentionally not stored here -
/// with `{#if X is T}` arms, different concrete `T`s can route through
/// different pragmas (or no pragma, falling through to local-body
/// lowering). The classification is recomputed per-`T` in
/// [`monomorphize`] from the expanded body.
#[derive(Clone)]
pub struct AnyFunctionInfo<'db> {
    pub func: Function<'db>,
    /// The ANY_* spec on the return type (or first ANY_* param).
    pub any_spec: ElementarySpec,
}

/// Detect whether a function has ANY_* typed parameters or return type.
pub fn detect_any_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<AnyFunctionInfo<'db>> {
    // Check return type first
    if let Some(ret) = func.return_type(db)
        && let Type::Elementary(e) = ret.infer(db)
        && e.is_any()
    {
        return Some(AnyFunctionInfo { func, any_spec: e });
    }

    // Check parameters
    let scope_id = func.scope_id(db);
    let def_map = scope_id.def_map(db);
    for (_name, var) in &def_map.local_variables {
        if let Type::Elementary(e) = var.spec(db).infer(db)
            && e.is_any()
        {
            return Some(AnyFunctionInfo { func, any_spec: e });
        }
    }

    None
}

/// Resolve `{#if X is T}` arms against a concrete type, returning a flat
/// statement list for that monomorphization.
///
/// For each `PreprocessIf`, the first arm whose `cond.expected` resolves
/// to `concrete` is inlined recursively (arms can nest). If no arm
/// matches, the entire `PreprocessIf` is dropped - the implementer's
/// responsibility to cover variants they care about. All other
/// statements pass through unchanged.
///
/// This mirrors Zig's `zirCondbr`: when the cond is a known value, only
/// the matching arm is analyzed; non-taken arms are never visited.
pub(crate) fn expanded_body_for_concrete<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
    concrete: ElementarySpec,
) -> Vec<hir::hir_def::expressions::statement::Stmt<'db>> {
    use hir::hir_def::expressions::spec::SpecKind;
    use hir::hir_def::expressions::statement::StmtKind;
    let mut result = Vec::new();
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::PreprocessIf { branches } => {
                for branch in branches {
                    // The cond's expected spec lives in the body, not in
                    // the signature, so `Spec::infer` returns `Never`
                    // (it only knows about signature-level specs). Read
                    // the kind directly. E0327 already rejected anything
                    // other than a concrete elementary, so a `Simple`
                    // match is the only thing we need to handle.
                    if let SpecKind::Simple(e) = branch.cond.expected.kind(db)
                        && *e == concrete
                    {
                        result.extend(expanded_body_for_concrete(db, &branch.body, concrete));
                        break;
                    }
                }
            }
            _ => result.push(*stmt),
        }
    }
    result
}

/// Find extern pragma in a flat statement list.
fn find_extern_decl_in<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
) -> Option<ExternDecl<'db>> {
    use hir::hir_def::expressions::statement::StmtKind;
    stmts.iter().find_map(|stmt| {
        if let StmtKind::ExternPragma(decl) = stmt.stmt(db) {
            Some(decl.clone())
        } else {
            None
        }
    })
}

/// One concrete instantiation of a (generic) FB.
///
/// For non-generic FBs, `subs` is empty and `mangled_name == fb.name(db)`.
/// For generic FBs, `subs` carries each ANY_* field's concrete binding,
/// and `mangled_name` interleaves the concretes (e.g. `Counter$INT`).
#[derive(Clone)]
pub struct FbInstance<'db> {
    pub fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
    pub subs: FxHashMap<Ident, ElementarySpec>,
    pub mangled_name: Ident,
}

/// Per-variable lookup from a `VariableDecl` to the mangled FB name of
/// its concrete instantiation, plus the underlying instances.
///
/// Replaces the legacy `fb_subs: Map<fb_name, Map<field, concrete>>`
/// shape, which collapsed all instantiations of one FB into a single
/// substitution map (last-write-wins).
#[derive(Default, Clone)]
pub struct FbInstanceMap<'db> {
    pub var_to_mangled: FxHashMap<hir::hir_def::pous::variable::VariableDecl<'db>, Ident>,
    pub by_mangled: FxHashMap<Ident, FbInstance<'db>>,
}

impl<'db> FbInstanceMap<'db> {
    pub fn from_instances(
        instances: Vec<FbInstance<'db>>,
        var_to_mangled: FxHashMap<hir::hir_def::pous::variable::VariableDecl<'db>, Ident>,
    ) -> Self {
        let mut by_mangled = FxHashMap::default();
        for inst in instances {
            by_mangled.insert(inst.mangled_name, inst);
        }
        FbInstanceMap {
            var_to_mangled,
            by_mangled,
        }
    }

    /// Mangled name for a variable's FB instantiation, if it's an FB
    /// instance. Returns `None` for non-FB variables and for FB
    /// variables that the collector didn't tag (e.g. unused or
    /// non-generic FBs that fall back to bare names elsewhere).
    pub fn mangled_for_var(
        &self,
        var: hir::hir_def::pous::variable::VariableDecl<'db>,
    ) -> Option<Ident> {
        self.var_to_mangled.get(&var).copied()
    }

    /// Look up the substitution map for a mangled name.
    pub fn subs_for_mangled(&self, mangled: Ident) -> Option<&FxHashMap<Ident, ElementarySpec>> {
        self.by_mangled.get(&mangled).map(|i| &i.subs)
    }
}

/// Mangle a generic FB instantiation into a unique name.
///
/// - Non-generic (empty subs) → returns `fb_name` unchanged so existing
///   names like `Counter$__body__` keep working for non-generic FBs.
/// - Generic (one or more subs) → joins concretes by `$` in
///   field-name-sorted order to make the mangling deterministic.
pub fn mangle_fb_name(
    db: &dyn WorkspaceDataBase,
    fb_name: Ident,
    subs: &FxHashMap<Ident, ElementarySpec>,
) -> Ident {
    let mut entries: Vec<_> = subs.iter().collect();
    entries.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
    let parts: Vec<&str> = entries.iter().map(|(_, c)| c.type_name()).collect();
    mangle_generic_name(db, fb_name, &parts)
}

/// Build a monomorphization symbol `base$Part1$Part2…` from already-sorted
/// concrete part names, or `base` unchanged when there are no parts. Shared by
/// FB-instance mangling ([`mangle_fb_name`]) and interface-param specialization
/// (`mono_iface`) so the `$`-suffix convention can't drift between them.
pub fn mangle_generic_name(db: &dyn WorkspaceDataBase, base: Ident, parts: &[&str]) -> Ident {
    if parts.is_empty() {
        return base;
    }
    let mut s = base.text(db).to_string();
    for p in parts {
        s.push('$');
        s.push_str(p);
    }
    Ident::new(db, compact_str::CompactString::from(s))
}

/// The namespace-qualified name of a POU (bare for a top-level one): the
/// canonical MIR identifier, so `NsA.foo` and `NsB.foo` stay distinct.
pub fn qualified_pou_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: hir::hir_ty::ty::Type<'db>,
) -> Ident {
    use hir::HirNodeInfo;
    use hir::hir_def::scope::ScopeKind;
    use hir::hir_def::semantic_index::semantic_index;

    let (scope_id, bare) = match ty {
        hir::hir_ty::ty::Type::Function(f) => (f.get_scope_id(db), f.name(db)),
        hir::hir_ty::ty::Type::FunctionBlock(fb) => (fb.get_scope_id(db), fb.name(db)),
        hir::hir_ty::ty::Type::Class(c) => (c.get_scope_id(db), c.name(db)),
        hir::hir_ty::ty::Type::Interface(i) => (i.get_scope_id(db), i.name(db)),
        hir::hir_ty::ty::Type::DataType(dt) => (dt.get_scope_id(db), dt.name(db)),
        hir::hir_ty::ty::Type::Program(p) => (p.get_scope_id(db), p.name(db)),
        _ => return Ident::new(db, compact_str::CompactString::from("")),
    };

    if scope_id.is_global(db) {
        return bare;
    }

    let sema = semantic_index(db, scope_id.file(db));
    for scope in sema.scope_iterator(db, scope_id) {
        if let ScopeKind::Namespace(ns) = scope.kind {
            let ns_str = ns.path(db).to_string(db);
            return Ident::new(
                db,
                compact_str::CompactString::from(format!("{}.{}", ns_str, bare.text(db))),
            );
        }
    }
    bare
}

/// Walk every body's `fb_any_resolutions` and collect the unique set of
/// FB instantiations actually used at call sites.
///
/// Returns:
/// - `instances`: list of unique `FbInstance`s (by `(fb_name,
///   sorted_subs)`); also includes a default empty-subs instantiation
///   for every non-generic FB so the existing struct/body emission path
///   stays uniform.
/// - `var_to_mangled`: per-`VariableDecl` lookup mapping each FB-instance
///   variable to its mangled FB name. Used to type variables and route
///   call sites to the correct `__body__`.
pub fn collect_fb_instantiations<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&hir::hir_def::pous::pou::Pou<'db>, Option<String>)],
) -> (
    Vec<FbInstance<'db>>,
    FxHashMap<hir::hir_def::pous::variable::VariableDecl<'db>, Ident>,
) {
    use hir::hir_def::pous::pou::Pou;
    use hir::hir_ty::body::infer_body;
    use hir::hir_ty::infer::Infer;

    // Canonical key: (fb_name, sorted concrete bindings as a Vec).
    let mut by_canonical: rustc_hash::FxHashMap<(Ident, Vec<(Ident, ElementarySpec)>), Ident> =
        FxHashMap::default();
    let mut instances: Vec<FbInstance<'db>> = Vec::new();
    let mut var_to_mangled: FxHashMap<hir::hir_def::pous::variable::VariableDecl<'db>, Ident> =
        FxHashMap::default();

    // Walk every POU body's fb_any_resolutions, group by VariableDecl,
    // and collect unique (fb, subs) tuples.
    for (pou, _) in all_pous {
        let scope = match pou {
            Pou::Function(f) => f.scope_id(db),
            Pou::FunctionBlock(fb) => fb.scope_id(db),
            Pou::Class(c) => c.scope_id(db),
            _ => continue,
        };

        // Also walk method scopes inside FBs/Classes so per-method
        // bodies' instantiations are seen.
        let body = infer_body(db, scope);
        let mut per_var: FxHashMap<
            hir::hir_def::pous::variable::VariableDecl<'db>,
            FxHashMap<Ident, ElementarySpec>,
        > = FxHashMap::default();
        for ((var_decl, field), concrete) in &body.fb_any_resolutions {
            per_var
                .entry(*var_decl)
                .or_default()
                .insert(*field, *concrete);
        }

        for (var_decl, subs) in per_var {
            let var_ty = var_decl.spec(db).infer(db).normalize(db);
            let fb = match var_ty {
                hir::hir_ty::ty::Type::FunctionBlock(fb) => fb,
                _ => continue,
            };

            // Use the FB's namespace-qualified name as the mangling
            // basis so two same-named FBs in different namespaces don't
            // collide (e.g. `NsA.Counter$INT`, `NsB.Counter$INT`).
            let qualified = qualified_pou_ident(db, hir::hir_ty::ty::Type::FunctionBlock(fb));

            let mut sorted: Vec<(Ident, ElementarySpec)> =
                subs.iter().map(|(k, v)| (*k, *v)).collect();
            sorted.sort_by(|a, b| a.0.text(db).cmp(b.0.text(db)));
            let key = (qualified, sorted);

            let mangled = match by_canonical.get(&key) {
                Some(m) => *m,
                None => {
                    let m = mangle_fb_name(db, qualified, &subs);
                    by_canonical.insert(key, m);
                    instances.push(FbInstance {
                        fb,
                        subs: subs.clone(),
                        mangled_name: m,
                    });
                    m
                }
            };
            var_to_mangled.insert(var_decl, mangled);
        }
    }

    // Add a default (empty-subs) instantiation for every non-generic FB
    // so the lowering loop can iterate uniformly over `instances`.
    for (pou, _) in all_pous {
        if let Pou::FunctionBlock(fb) = pou {
            let has_any = fb.variables(db).iter().any(|v| {
                let ty = v.spec(db).infer(db).normalize(db);
                matches!(ty, hir::hir_ty::ty::Type::Elementary(e) if e.is_any())
            });
            if has_any {
                continue;
            }
            let qualified = qualified_pou_ident(db, hir::hir_ty::ty::Type::FunctionBlock(*fb));
            let key = (qualified, Vec::new());
            if let std::collections::hash_map::Entry::Vacant(e) = by_canonical.entry(key) {
                e.insert(qualified);
                instances.push(FbInstance {
                    fb: *fb,
                    subs: FxHashMap::default(),
                    mangled_name: qualified,
                });
            }
        }
    }

    (instances, var_to_mangled)
}

/// Find wasm pragma in a flat statement list.
fn find_wasm_decl_in<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
) -> Option<hir::hir_def::extern_decl::WasmDecl<'db>> {
    use hir::hir_def::expressions::statement::StmtKind;
    stmts.iter().find_map(|stmt| {
        if let StmtKind::WasmPragma(decl) = stmt.stmt(db) {
            Some(decl.clone())
        } else {
            None
        }
    })
}

/// Get the set of concrete types for a given ANY_* spec.
pub fn concrete_types_for_any(any_spec: ElementarySpec) -> &'static [ElementarySpec] {
    match any_spec {
        ElementarySpec::AnyNum => &[
            ElementarySpec::SInt,
            ElementarySpec::Int,
            ElementarySpec::DInt,
            ElementarySpec::LInt,
            ElementarySpec::USInt,
            ElementarySpec::UInt,
            ElementarySpec::UDInt,
            ElementarySpec::ULInt,
            ElementarySpec::Real,
            ElementarySpec::LReal,
        ],
        ElementarySpec::AnyInt => &[
            ElementarySpec::SInt,
            ElementarySpec::Int,
            ElementarySpec::DInt,
            ElementarySpec::LInt,
            ElementarySpec::USInt,
            ElementarySpec::UInt,
            ElementarySpec::UDInt,
            ElementarySpec::ULInt,
        ],
        ElementarySpec::AnySigned => &[
            ElementarySpec::SInt,
            ElementarySpec::Int,
            ElementarySpec::DInt,
            ElementarySpec::LInt,
        ],
        ElementarySpec::AnyUnsigned => &[
            ElementarySpec::USInt,
            ElementarySpec::UInt,
            ElementarySpec::UDInt,
            ElementarySpec::ULInt,
        ],
        ElementarySpec::AnyReal => &[ElementarySpec::Real, ElementarySpec::LReal],
        ElementarySpec::AnyBit => &[
            ElementarySpec::Bool,
            ElementarySpec::Byte,
            ElementarySpec::Word,
            ElementarySpec::DWord,
            ElementarySpec::LWord,
        ],
        ElementarySpec::AnyMagnitude => &[
            ElementarySpec::SInt,
            ElementarySpec::Int,
            ElementarySpec::DInt,
            ElementarySpec::LInt,
            ElementarySpec::USInt,
            ElementarySpec::UInt,
            ElementarySpec::UDInt,
            ElementarySpec::ULInt,
            ElementarySpec::Real,
            ElementarySpec::LReal,
            ElementarySpec::Time,
            ElementarySpec::LTime,
        ],
        ElementarySpec::AnyElementary | ElementarySpec::Any => &[
            ElementarySpec::SInt,
            ElementarySpec::Int,
            ElementarySpec::DInt,
            ElementarySpec::LInt,
            ElementarySpec::USInt,
            ElementarySpec::UInt,
            ElementarySpec::UDInt,
            ElementarySpec::ULInt,
            ElementarySpec::Real,
            ElementarySpec::LReal,
            ElementarySpec::Bool,
            ElementarySpec::Byte,
            ElementarySpec::Word,
            ElementarySpec::DWord,
            ElementarySpec::LWord,
        ],
        _ => &[],
    }
}

/// Run monomorphization on a MirModule.
///
/// This processes all ANY_* functions:
/// - For extern ANY_*: generates `MirExternFunction` entries with suffixed import names
/// - For local ANY_*: clones the function body with type substitution
/// - Rewrites all call sites to point to the monomorphized copies
pub fn monomorphize<'db>(
    db: &'db dyn WorkspaceDataBase,
    module: &mut MirModule,
    any_functions: &[AnyFunctionInfo<'db>],
    _memory_layout: &mut MirMemoryLayout,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<(), LowerTypeError> {
    // Collect which ANY_* functions exist by qualified name. The
    // discovery walk matches `MirCall.callee` against this set, and
    // call sites carry namespace-qualified callee names (e.g.
    // `NsA.abs`) — so the set is keyed the same way.
    let any_func_names: FxHashSet<Ident> = any_functions
        .iter()
        .map(|info| qualified_pou_ident(db, hir::hir_ty::ty::Type::Function(info.func)))
        .collect();

    // Phase 1: Discover which concrete types are actually used at call sites.
    //
    // This walks the *currently-existing* functions in the module.
    // Generic functions (with ANY params) were already skipped by
    // `lower_module`, so we only see concrete callers (tests, etc.).
    // Generated monomorphizations may introduce new needs — those get
    // picked up by the worklist loop below, not here.
    let mut needed_instantiations: FxHashMap<Ident, FxHashSet<ElementarySpec>> =
        FxHashMap::default();

    for func in &module.functions {
        discover_calls_in_stmts(&func.body, &any_func_names, &mut needed_instantiations);
    }

    // Phase 2: Generate monomorphized copies — iteratively, until no new
    // `(any_func, concrete_type)` pairs surface.
    //
    // Why iterate: a monomorphized function's body can call *another*
    // ANY function with a concrete type that phase 1 never saw, because
    // phase 1 only knew about non-generic callers. The poster child
    // is `Std.Unit.ASSERT_EQ.BYTE`, whose body calls
    // `Std.Convert.ANY_TO_STRING(value:BYTE)`. Before the loop existed,
    // we'd generate `ASSERT_EQ.BYTE` (because something asserted BYTEs)
    // but not `ANY_TO_STRING.BYTE` (nobody called it with BYTE at
    // module-top-level), and phase 3 couldn't rewrite the call.
    //
    // The loop:
    //   1. Generate every `(func, spec)` pair present in
    //      `needed_instantiations` that isn't already in
    //      `generated_variants`.
    //   2. Scan the newly-pushed bodies for ANY calls and merge their
    //      needs back into `needed_instantiations`.
    //   3. Stop when an iteration produces zero new pairs.
    let mut mono_indices: FxHashMap<(Ident, MirElementary), (Ident, u32)> = FxHashMap::default();
    let mut next_fn_idx = module.functions.len() as u32 + module.extern_functions.len() as u32;
    let mut generated_variants: FxHashSet<(Ident, ElementarySpec)> = FxHashSet::default();

    loop {
        let funcs_at_iter_start = module.functions.len();

        for info in any_functions {
            // Use the qualified name as the canonical MIR identifier so
            // monomorphizations and call-site lookups stay aligned with
            // the rest of the module's `function_indices`.
            let func_name = qualified_pou_ident(db, hir::hir_ty::ty::Type::Function(info.func));

            // Get the concrete types actually needed (from call sites)
            // Fall back to all types in the ANY group if no calls found
            // (extern functions might be called from host)
            let concrete_types: Vec<ElementarySpec> = needed_instantiations
                .get(&func_name)
                .map(|set| set.iter().copied().collect())
                .unwrap_or_else(|| concrete_types_for_any(info.any_spec).to_vec());

            for concrete_spec in &concrete_types {
                // Skip variants we already generated in a previous worklist
                // iteration. Without this guard, each iteration would re-emit
                // every variant and push duplicate functions into the module.
                if !generated_variants.insert((func_name, *concrete_spec)) {
                    continue;
                }

                let mir_elem = match elementary_spec_to_mir(*concrete_spec) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let type_suffix = concrete_spec.type_name();
                let mono_name = Ident::new(
                    db,
                    CompactString::from(format!("{}.{}", func_name.text(db), type_suffix)),
                );

                // Resolve `{#if}` arms against this concrete type and classify
                // the resulting body. Arms that don't match `concrete_spec` are
                // dropped; arms that do match are inlined.
                let expanded =
                    expanded_body_for_concrete(db, info.func.statements(db), *concrete_spec);
                let wasm_decl_for_t = find_wasm_decl_in(db, &expanded);
                let extern_decl_for_t = find_extern_decl_in(db, &expanded);

                if let Some(wasm_decl) = &wasm_decl_for_t {
                    // Wasm intrinsic ANY_* function → build monomorphized function with concrete types

                    use crate::function::*;
                    use crate::stmt::*;
                    use hir::hir_def::pous::variable::VariableKind;

                    let concrete_mir = MirType::Elementary(mir_elem);

                    let mut params = Vec::new();
                    for var in info.func.variables(db) {
                        match var.kind(db) {
                            VariableKind::Input => {
                                let ty =
                                    resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                                params.push(MirParam {
                                    name: var.name(db),
                                    ty,
                                    kind: MirParamKind::Input,
                                });
                            }
                            VariableKind::Output => {
                                let ty =
                                    resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                                params.push(MirParam {
                                    name: var.name(db),
                                    ty: MirType::Pointer(Box::new(ty)),
                                    kind: MirParamKind::Output,
                                });
                            }
                            _ => {}
                        }
                    }

                    let return_type = info
                        .func
                        .return_type(db)
                        .map(|spec| resolve_any_type(db, spec.infer(db), *concrete_spec))
                        .transpose()?;

                    // Resolve the instruction name: if type_ref is set, prefix with the WASM type
                    let full_instruction = if wasm_decl.type_ref.is_some() {
                        // Determine WASM type prefix from the concrete monomorphized type
                        let prefix = if mir_elem.is_float() {
                            if mir_elem.is_64bit() { "f64" } else { "f32" }
                        } else if mir_elem.is_64bit() {
                            "i64"
                        } else {
                            "i32"
                        };
                        CompactString::from(format!("{}.{}", prefix, wasm_decl.instruction))
                    } else {
                        wasm_decl.instruction.clone()
                    };

                    let result_name = info.func.name(db);
                    let param_names: Vec<_> = params.iter().map(|p| p.name).collect();

                    let body = vec![MirStmt::WasmIntrinsic {
                        instruction: full_instruction,
                        params: param_names,
                        result: Some(result_name),
                    }];

                    // Allocate the return-slot storage. Scalars go into a
                    // wasm local, STRING/composite into memory. The wasm
                    // local-index for a scalar slot must reflect total
                    // wasm-slot width (STRING params count as 2), not
                    // logical param count — `param_wasm_width` knows the
                    // rule.
                    let mut next_local_idx: u32 = params
                        .iter()
                        .map(|p| crate::lower::lower_func::param_wasm_width(&p.ty, p.kind))
                        .sum();
                    let storage = crate::lower::lower_func::allocate_local_storage(
                        result_name,
                        &concrete_mir,
                        /* is_address_taken = */ false,
                        &mut next_local_idx,
                        &mut module.memory_layout,
                    );
                    let locals = vec![MirLocal {
                        name: result_name,
                        ty: concrete_mir.clone(),
                        init: None,
                        kind: MirLocalKind::Var,
                        storage,
                        // Monomorphized FUNCTION result slot — stateless.
                        var_storage: MirVariableStorage::Automatic,
                    }];

                    module.functions.push(MirFunction {
                        name: mono_name,
                        origin_name: info.func.name(db),
                        index: next_fn_idx,
                        params,
                        return_type,
                        locals,
                        body,
                        linkage: MirLinkage::Export,
                        is_test: false,
                        export_name: None,
                    });
                } else if let Some(extern_decl) = &extern_decl_for_t {
                    // Extern ANY_* function → generate MirExternFunction with suffixed name
                    let mut params = Vec::new();
                    for var in info.func.variables(db) {
                        use hir::hir_def::pous::variable::VariableKind;
                        match var.kind(db) {
                            VariableKind::Input => {
                                let ty =
                                    resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                                params.push(MirParam {
                                    name: var.name(db),
                                    ty,
                                    kind: MirParamKind::Input,
                                });
                            }
                            VariableKind::InOut | VariableKind::Output => {
                                let ty =
                                    resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                                let kind = if var.kind(db) == VariableKind::InOut {
                                    MirParamKind::InOut
                                } else {
                                    MirParamKind::Output
                                };
                                params.push(MirParam {
                                    name: var.name(db),
                                    ty: MirType::Pointer(Box::new(ty)),
                                    kind,
                                });
                            }
                            _ => {}
                        }
                    }

                    let return_type = info
                        .func
                        .return_type(db)
                        .map(|spec| resolve_any_type(db, spec.infer(db), *concrete_spec))
                        .transpose()?;

                    let import_name =
                        CompactString::from(format!("{}.{}", &extern_decl.name, type_suffix));

                    module.extern_functions.push(MirExternFunction {
                        name: mono_name,
                        index: next_fn_idx,
                        module: extern_decl.module.clone(),
                        import_name,
                        params,
                        return_type,
                        monomorphized_from: Some(func_name),
                    });
                } else {
                    // Skip variadic functions (they're inlined at call sites)
                    let scope_id = info.func.scope_id(db);
                    let def_map = scope_id.def_map(db);
                    let has_variadic = def_map.local_variables.values().any(|v| v.variadic(db));
                    if has_variadic {
                        continue;
                    }

                    // Local ANY_* function → clone body with concrete types
                    let mono_func = lower_monomorphized_local(
                        db,
                        info.func,
                        mono_name,
                        *concrete_spec,
                        next_fn_idx,
                        &mut module.memory_layout,
                        string_pool.clone(),
                    )?;
                    module.functions.push(mono_func);
                }

                module.function_indices.insert(mono_name, next_fn_idx);
                mono_indices.insert((func_name, mir_elem), (mono_name, next_fn_idx));
                next_fn_idx += 1;
            }
        }

        // Did this iteration push any new functions? If not, fixed
        // point reached.
        if module.functions.len() == funcs_at_iter_start {
            break;
        }

        // Walk the newly-generated bodies and merge any newly-discovered
        // ANY-call needs back into `needed_instantiations`. Next loop
        // iteration will see them and generate the corresponding
        // monomorphizations.
        for func in &module.functions[funcs_at_iter_start..] {
            discover_calls_in_stmts(&func.body, &any_func_names, &mut needed_instantiations);
        }
    }

    // Phase 3: Rewrite call sites in all functions
    for func in &mut module.functions {
        rewrite_calls_in_stmts(&mut func.body, &any_func_names, &mono_indices);
    }

    Ok(())
}

/// Lower a local ANY_* function with a concrete type override.
fn lower_monomorphized_local<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    mono_name: Ident,
    concrete_spec: ElementarySpec,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<MirFunction, LowerTypeError> {
    use hir::hir_def::pous::variable::VariableKind;

    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;

    for var in func.variables(db) {
        let raw_type = var.spec(db).infer(db);
        let ty = resolve_any_type(db, raw_type, concrete_spec)?;

        match var.kind(db) {
            VariableKind::Input => {
                let param = MirParam {
                    name: var.name(db),
                    ty,
                    kind: MirParamKind::Input,
                };
                next_local_idx += super::lower_func::param_wasm_width(&param.ty, param.kind);
                params.push(param);
            }
            VariableKind::InOut | VariableKind::Output => {
                let kind = if var.kind(db) == VariableKind::InOut {
                    MirParamKind::InOut
                } else {
                    MirParamKind::Output
                };
                let param = MirParam {
                    name: var.name(db),
                    ty: MirType::Pointer(Box::new(ty)),
                    kind,
                };
                next_local_idx += super::lower_func::param_wasm_width(&param.ty, param.kind);
                params.push(param);
            }
            VariableKind::Var | VariableKind::Temp => {
                let storage = super::lower_func::allocate_local_storage(
                    var.name(db),
                    &ty,
                    /* is_address_taken = */ false,
                    &mut next_local_idx,
                    memory_layout,
                );
                locals.push(MirLocal {
                    name: var.name(db),
                    ty,
                    init: None,
                    kind: MirLocalKind::Var,
                    storage,
                    // Monomorphized FUNCTION local — stateless.
                    var_storage: MirVariableStorage::Automatic,
                });
            }
            other => unreachable!(
                "unexpected variable kind {:?} in monomorphized function",
                other
            ),
        }
    }

    let return_type = func
        .return_type(db)
        .map(|spec| resolve_any_type(db, spec.infer(db), concrete_spec))
        .transpose()?;

    if let Some(ref ret_ty) = return_type {
        let storage = super::lower_func::allocate_local_storage(
            func.name(db),
            ret_ty,
            /* is_address_taken = */ false,
            &mut next_local_idx,
            memory_layout,
        );
        locals.push(MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: MirLocalKind::Var,
            storage,
            // Monomorphized FUNCTION return slot — stateless.
            var_storage: MirVariableStorage::Automatic,
        });
    }

    // Resolve `{#if}` arms against this concrete type, then lower the
    // resulting flat body with the ANY override.
    let expanded = expanded_body_for_concrete(db, func.statements(db), concrete_spec);
    let body = crate::lower::lower_stmt::lower_stmts_with_ctx(
        db,
        &expanded,
        Some(concrete_spec),
        None,
        string_pool,
    )?;

    Ok(MirFunction {
        name: mono_name,
        origin_name: func.name(db),
        index,
        params,
        return_type,
        locals,
        body,
        linkage: MirLinkage::Export,
        is_test: false,
        export_name: None,
    })
}

/// Resolve a type, substituting ANY_* with the concrete type.
fn resolve_any_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    concrete_spec: ElementarySpec,
) -> Result<MirType, LowerTypeError> {
    let normalized = ty.normalize(db);
    match normalized {
        Type::Elementary(e) if e.is_any() => {
            let mir = elementary_spec_to_mir(concrete_spec)?;
            Ok(MirType::Elementary(mir))
        }
        _ => lower_type(db, normalized),
    }
}

// =============================================================================
// Discovery: walk MIR statements/expressions to find calls to ANY_* functions
// =============================================================================

fn discover_calls_in_stmts(
    stmts: &[MirStmt],
    any_names: &FxHashSet<Ident>,
    out: &mut FxHashMap<Ident, FxHashSet<ElementarySpec>>,
) {
    for stmt in stmts {
        discover_calls_in_stmt(stmt, any_names, out);
    }
}

fn discover_calls_in_stmt(
    stmt: &MirStmt,
    any_names: &FxHashSet<Ident>,
    out: &mut FxHashMap<Ident, FxHashSet<ElementarySpec>>,
) {
    match stmt {
        MirStmt::Assign { value, .. } => discover_calls_in_expr(value, any_names, out),
        MirStmt::Call(call) => discover_calls_in_call(call, any_names, out),
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            discover_calls_in_expr(condition, any_names, out);
            discover_calls_in_stmts(then_body, any_names, out);
            for (cond, body) in else_ifs {
                discover_calls_in_expr(cond, any_names, out);
                discover_calls_in_stmts(body, any_names, out);
            }
            if let Some(body) = else_body {
                discover_calls_in_stmts(body, any_names, out);
            }
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            discover_calls_in_expr(selector, any_names, out);
            for arm in arms {
                discover_calls_in_stmts(&arm.body, any_names, out);
            }
            if let Some(body) = else_body {
                discover_calls_in_stmts(body, any_names, out);
            }
        }
        MirStmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            discover_calls_in_expr(start, any_names, out);
            discover_calls_in_expr(end, any_names, out);
            discover_calls_in_expr(step, any_names, out);
            discover_calls_in_stmts(body, any_names, out);
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            discover_calls_in_expr(condition, any_names, out);
            discover_calls_in_stmts(body, any_names, out);
        }
        MirStmt::FbCall { input_writes, .. } => {
            for (_, value, _) in input_writes {
                discover_calls_in_expr(value, any_names, out);
            }
        }
        MirStmt::WasmIntrinsic { .. } => {}
        _ => {}
    }
}

fn discover_calls_in_expr(
    expr: &MirExpr,
    any_names: &FxHashSet<Ident>,
    out: &mut FxHashMap<Ident, FxHashSet<ElementarySpec>>,
) {
    match expr {
        MirExpr::Call(call) => discover_calls_in_call(call, any_names, out),
        MirExpr::BinOp { lhs, rhs, .. } => {
            discover_calls_in_expr(lhs, any_names, out);
            discover_calls_in_expr(rhs, any_names, out);
        }
        MirExpr::UnaryOp { expr, .. } => discover_calls_in_expr(expr, any_names, out),
        MirExpr::Cast { expr, .. } => discover_calls_in_expr(expr, any_names, out),
        _ => {}
    }
}

fn discover_calls_in_call(
    call: &MirCall,
    any_names: &FxHashSet<Ident>,
    out: &mut FxHashMap<Ident, FxHashSet<ElementarySpec>>,
) {
    // Recurse into arguments
    for arg in &call.args {
        discover_calls_in_expr(&arg.value, any_names, out);
    }

    // Check if this call targets an ANY_* function
    if !any_names.contains(&call.callee) {
        return;
    }

    // Determine concrete type from the call's return type (resolved by HIR at call site)
    // or fall back to first argument. Prefer non-BOOL (for SEL's G param), but
    // fall back to BOOL if all by-value args are BOOL.
    let concrete = match &call.return_type {
        MirType::Elementary(e) => Some(*e),
        _ => {
            let non_bool = call
                .args
                .iter()
                .filter(|a| a.kind == MirArgKind::ByValue)
                .find_map(|a| infer_concrete_type_from_expr(&a.value))
                .filter(|e| !matches!(e, MirElementary::Bool));
            non_bool.or_else(|| {
                call.args
                    .iter()
                    .filter(|a| a.kind == MirArgKind::ByValue)
                    .find_map(|a| infer_concrete_type_from_expr(&a.value))
            })
        }
    };

    if let Some(concrete) = concrete
        && let Some(spec) = mir_elementary_to_spec(concrete)
    {
        out.entry(call.callee).or_default().insert(spec);
    }
}

/// Try to determine the concrete elementary type of a MIR expression.
fn infer_concrete_type_from_expr(expr: &MirExpr) -> Option<MirElementary> {
    match expr {
        MirExpr::Constant(c) => match c {
            MirConstant::Bool(_) => Some(MirElementary::Bool),
            MirConstant::I32(_) => Some(MirElementary::Int),
            MirConstant::I64(_) => Some(MirElementary::LInt),
            MirConstant::F32(_) => Some(MirElementary::Real),
            MirConstant::F64(_) => Some(MirElementary::LReal),
            MirConstant::Null => None,
        },
        MirExpr::BinOp { ty, .. } => Some(*ty),
        MirExpr::UnaryOp { ty, .. } => Some(*ty),
        MirExpr::Cast { to, .. } => Some(*to),
        // For loads and calls, we'd need type context - but the expression
        // was already typed during lowering. The call's return_type carries it.
        MirExpr::Call(call) => match &call.return_type {
            MirType::Elementary(e) => Some(*e),
            _ => None,
        },
        MirExpr::Load(_, MirType::Elementary(e)) => Some(*e),
        _ => None,
    }
}

/// Map MirElementary back to ElementarySpec (for discovery phase).
fn mir_elementary_to_spec(elem: MirElementary) -> Option<ElementarySpec> {
    Some(match elem {
        MirElementary::Bool => ElementarySpec::Bool,
        MirElementary::SInt => ElementarySpec::SInt,
        MirElementary::Int => ElementarySpec::Int,
        MirElementary::DInt => ElementarySpec::DInt,
        MirElementary::LInt => ElementarySpec::LInt,
        MirElementary::USInt => ElementarySpec::USInt,
        MirElementary::UInt => ElementarySpec::UInt,
        MirElementary::UDInt => ElementarySpec::UDInt,
        MirElementary::ULInt => ElementarySpec::ULInt,
        MirElementary::Byte => ElementarySpec::Byte,
        MirElementary::Word => ElementarySpec::Word,
        MirElementary::DWord => ElementarySpec::DWord,
        MirElementary::LWord => ElementarySpec::LWord,
        MirElementary::Real => ElementarySpec::Real,
        MirElementary::LReal => ElementarySpec::LReal,
        MirElementary::Char => ElementarySpec::Char,
        MirElementary::Time => ElementarySpec::Time,
        MirElementary::LTime => ElementarySpec::LTime,
        MirElementary::Date => ElementarySpec::Date,
        MirElementary::LDate => ElementarySpec::LDate,
        MirElementary::Tod => ElementarySpec::Tod,
        MirElementary::LTod => ElementarySpec::LTod,
        MirElementary::DateAndTime => ElementarySpec::DateAndTime,
        MirElementary::LDateTime => ElementarySpec::LDateTime,
    })
}

// =============================================================================
// Rewriting: update call sites to reference monomorphized function indices
// =============================================================================

fn rewrite_calls_in_stmts(
    stmts: &mut [MirStmt],
    any_names: &FxHashSet<Ident>,
    mono_indices: &FxHashMap<(Ident, MirElementary), (Ident, u32)>,
) {
    for stmt in stmts.iter_mut() {
        rewrite_calls_in_stmt(stmt, any_names, mono_indices);
    }
}

fn rewrite_calls_in_stmt(
    stmt: &mut MirStmt,
    any_names: &FxHashSet<Ident>,
    mono_indices: &FxHashMap<(Ident, MirElementary), (Ident, u32)>,
) {
    match stmt {
        MirStmt::Assign { value, .. } => rewrite_calls_in_expr(value, any_names, mono_indices),
        MirStmt::Call(call) => rewrite_call(call, any_names, mono_indices),
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            rewrite_calls_in_expr(condition, any_names, mono_indices);
            rewrite_calls_in_stmts(then_body, any_names, mono_indices);
            for (cond, body) in else_ifs.iter_mut() {
                rewrite_calls_in_expr(cond, any_names, mono_indices);
                rewrite_calls_in_stmts(body, any_names, mono_indices);
            }
            if let Some(body) = else_body {
                rewrite_calls_in_stmts(body, any_names, mono_indices);
            }
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            rewrite_calls_in_expr(selector, any_names, mono_indices);
            for arm in arms.iter_mut() {
                rewrite_calls_in_stmts(&mut arm.body, any_names, mono_indices);
            }
            if let Some(body) = else_body {
                rewrite_calls_in_stmts(body, any_names, mono_indices);
            }
        }
        MirStmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            rewrite_calls_in_expr(start, any_names, mono_indices);
            rewrite_calls_in_expr(end, any_names, mono_indices);
            rewrite_calls_in_expr(step, any_names, mono_indices);
            rewrite_calls_in_stmts(body, any_names, mono_indices);
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            rewrite_calls_in_expr(condition, any_names, mono_indices);
            rewrite_calls_in_stmts(body, any_names, mono_indices);
        }
        MirStmt::FbCall { input_writes, .. } => {
            for (_, value, _) in input_writes.iter_mut() {
                rewrite_calls_in_expr(value, any_names, mono_indices);
            }
        }
        MirStmt::WasmIntrinsic { .. } => {} // no nested calls
        _ => {}
    }
}

fn rewrite_calls_in_expr(
    expr: &mut MirExpr,
    any_names: &FxHashSet<Ident>,
    mono_indices: &FxHashMap<(Ident, MirElementary), (Ident, u32)>,
) {
    match expr {
        MirExpr::Call(call) => rewrite_call(call, any_names, mono_indices),
        MirExpr::BinOp { lhs, rhs, .. } => {
            rewrite_calls_in_expr(lhs, any_names, mono_indices);
            rewrite_calls_in_expr(rhs, any_names, mono_indices);
        }
        MirExpr::UnaryOp { expr, .. } => rewrite_calls_in_expr(expr, any_names, mono_indices),
        MirExpr::Cast { expr, .. } => rewrite_calls_in_expr(expr, any_names, mono_indices),
        _ => {}
    }
}

fn rewrite_call(
    call: &mut MirCall,
    any_names: &FxHashSet<Ident>,
    mono_indices: &FxHashMap<(Ident, MirElementary), (Ident, u32)>,
) {
    // Recurse into arguments first
    for arg in &mut call.args {
        rewrite_calls_in_expr(&mut arg.value, any_names, mono_indices);
    }

    // If this call targets an ANY_* function, rewrite it
    if !any_names.contains(&call.callee) {
        return;
    }

    // Determine concrete type from return type or first argument.
    // Prefer non-BOOL args (for functions like SEL where G:BOOL is not the generic type),
    // but fall back to BOOL if all args are BOOL.
    let concrete = match &call.return_type {
        MirType::Elementary(e) => Some(*e),
        _ => {
            let non_bool = call
                .args
                .iter()
                .filter(|a| a.kind == MirArgKind::ByValue)
                .find_map(|a| infer_concrete_type_from_expr(&a.value))
                .filter(|e| !matches!(e, MirElementary::Bool));
            non_bool.or_else(|| {
                call.args
                    .iter()
                    .filter(|a| a.kind == MirArgKind::ByValue)
                    .find_map(|a| infer_concrete_type_from_expr(&a.value))
            })
        }
    };

    if let Some(concrete_elem) = concrete
        && let Some(&(mono_name, new_idx)) = mono_indices.get(&(call.callee, concrete_elem))
    {
        call.callee = mono_name;
        call.callee_index = new_idx;
    }
}
