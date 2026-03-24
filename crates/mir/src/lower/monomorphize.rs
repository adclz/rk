//! Monomorphization of ANY_* functions.
//!
//! ANY_* functions are polymorphic — they accept parameters typed as AnyInt, AnyReal, etc.
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
        expressions::spec::ElementarySpec,
        extern_decl::ExternDecl,
        interned::identifier::Ident,
        pous::function::Function,
    },
    hir_ty::{infer::Infer, ty::Type},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    MirModule,
    expr::{MirArgKind, MirCall, MirConstant, MirExpr},
    function::{
        MirExternFunction, MirFunction, MirLinkage, MirLocal, MirLocalKind, MirParam,
        MirParamKind, MirStorage,
    },
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir, lower_type},
    memory::{MirAllocKind, MirMemoryLayout},
    stmt::MirStmt,
    types::{MirElementary, MirType},
};

/// Info about an ANY_* function pending monomorphization.
#[derive(Clone)]
pub struct AnyFunctionInfo<'db> {
    pub func: Function<'db>,
    /// The ANY_* spec on the return type (or first ANY_* param).
    pub any_spec: ElementarySpec,
    /// If extern, the extern declaration.
    pub extern_decl: Option<ExternDecl<'db>>,
    /// If wasm intrinsic, the wasm declaration.
    pub wasm_decl: Option<hir::hir_def::extern_decl::WasmDecl<'db>>,
}

/// Detect whether a function has ANY_* typed parameters or return type.
pub fn detect_any_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<AnyFunctionInfo<'db>> {
    let make_info = |e: ElementarySpec| {
        let extern_decl = find_extern_decl(db, func);
        let wasm_decl = find_wasm_decl(db, func);
        AnyFunctionInfo { func, any_spec: e, extern_decl, wasm_decl }
    };

    // Check return type first
    if let Some(ret) = func.return_type(db) {
        if let Type::Elementary(e) = ret.infer(db) {
            if e.is_any() {
                return Some(make_info(e));
            }
        }
    }

    // Check parameters
    let scope_id = func.scope_id(db);
    let def_map = scope_id.def_map(db);
    for (_name, var) in &def_map.local_variables {
        if let Type::Elementary(e) = var.spec(db).infer(db) {
            if e.is_any() {
                return Some(make_info(e));
            }
        }
    }

    None
}

/// Find extern pragma in a function's statements.
fn find_extern_decl<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<ExternDecl<'db>> {
    use hir::hir_def::expressions::statement::StmtKind;
    func.statements(db).iter().find_map(|stmt| {
        if let StmtKind::ExternPragma(decl) = stmt.stmt(db) {
            Some(decl.clone())
        } else {
            None
        }
    })
}

/// Find wasm pragma in a function's statements.
fn find_wasm_decl<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<hir::hir_def::extern_decl::WasmDecl<'db>> {
    use hir::hir_def::expressions::statement::StmtKind;
    func.statements(db).iter().find_map(|stmt| {
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
            ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ElementarySpec::Real, ElementarySpec::LReal,
        ],
        ElementarySpec::AnyInt => &[
            ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
        ],
        ElementarySpec::AnySigned => &[
            ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
        ],
        ElementarySpec::AnyUnsigned => &[
            ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
        ],
        ElementarySpec::AnyReal => &[ElementarySpec::Real, ElementarySpec::LReal],
        ElementarySpec::AnyBit => &[
            ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
            ElementarySpec::DWord, ElementarySpec::LWord,
        ],
        ElementarySpec::AnyMagnitude => &[
            ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ElementarySpec::Real, ElementarySpec::LReal,
            ElementarySpec::Time, ElementarySpec::LTime,
        ],
        ElementarySpec::AnyElementary | ElementarySpec::Any => &[
            ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ElementarySpec::Real, ElementarySpec::LReal,
            ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
            ElementarySpec::DWord, ElementarySpec::LWord,
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
    memory_layout: &mut MirMemoryLayout,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<(), LowerTypeError> {
    // Collect which ANY_* functions exist by name
    let any_func_names: FxHashSet<Ident> = any_functions
        .iter()
        .map(|info| info.func.name(db))
        .collect();

    // Phase 1: Discover which concrete types are actually used at call sites
    let mut needed_instantiations: FxHashMap<Ident, FxHashSet<ElementarySpec>> =
        FxHashMap::default();

    for func in &module.functions {
        discover_calls_in_stmts(&func.body, &any_func_names, &mut needed_instantiations);
    }

    // Phase 2: Generate monomorphized copies
    // Maps (original_name, concrete_type) → (monomorphized_name, fn_index)
    let mut mono_indices: FxHashMap<(Ident, MirElementary), (Ident, u32)> = FxHashMap::default();
    let mut next_fn_idx = module.functions.len() as u32 + module.extern_functions.len() as u32;

    for info in any_functions {
        let func_name = info.func.name(db);

        // Get the concrete types actually needed (from call sites)
        // Fall back to all types in the ANY group if no calls found
        // (extern functions might be called from host)
        let concrete_types: Vec<ElementarySpec> = needed_instantiations
            .get(&func_name)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_else(|| concrete_types_for_any(info.any_spec).to_vec());

        for concrete_spec in &concrete_types {
            let mir_elem = match elementary_spec_to_mir(*concrete_spec) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let type_suffix = concrete_spec.type_name();
            let mono_name = Ident::new(
                db,
                CompactString::from(format!("{}.{}", func_name.text(db), type_suffix)),
            );

            if let Some(wasm_decl) = &info.wasm_decl {
                // Wasm intrinsic ANY_* function → build monomorphized function with concrete types
                use crate::function::*;
                use crate::expr::*;
                use crate::stmt::*;
                use hir::hir_def::pous::variable::VariableKind;

                let concrete_mir = MirType::Elementary(mir_elem);

                let mut params = Vec::new();
                for var in info.func.variables(db) {
                    match var.kind(db) {
                        VariableKind::Input => {
                            let ty = resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                            params.push(MirParam {
                                name: var.name(db),
                                ty,
                                kind: MirParamKind::Input,
                            });
                        }
                        VariableKind::Output => {
                            let ty = resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                            params.push(MirParam {
                                name: var.name(db),
                                ty: MirType::Pointer(Box::new(ty)),
                                kind: MirParamKind::Output,
                            });
                        }
                        _ => {}
                    }
                }

                let return_type = info.func
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

                let locals = vec![MirLocal {
                    name: result_name,
                    ty: concrete_mir.clone(),
                    init: None,
                    kind: MirLocalKind::Var,
                    storage: MirStorage::Scalar { local_index: params.len() as u32 },
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
                    export_name: None,
                });
            } else if let Some(extern_decl) = &info.extern_decl {
                // Extern ANY_* function → generate MirExternFunction with suffixed name
                let mut params = Vec::new();
                for var in info.func.variables(db) {
                    use hir::hir_def::pous::variable::VariableKind;
                    match var.kind(db) {
                        VariableKind::Input => {
                            let ty = resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
                            params.push(MirParam {
                                name: var.name(db),
                                ty,
                                kind: MirParamKind::Input,
                            });
                        }
                        VariableKind::InOut | VariableKind::Output => {
                            let ty = resolve_any_type(db, var.spec(db).infer(db), *concrete_spec)?;
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

                let import_name = CompactString::from(format!(
                    "{}.{}",
                    &extern_decl.name,
                    type_suffix
                ));

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
                let has_variadic = def_map.local_variables.values()
                    .any(|v| v.variadic(db));
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
                params.push(MirParam {
                    name: var.name(db),
                    ty,
                    kind: MirParamKind::Input,
                });
                next_local_idx += 1;
            }
            VariableKind::InOut | VariableKind::Output => {
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
                next_local_idx += 1;
            }
            VariableKind::Var | VariableKind::Temp => {
                let storage = if ty.is_scalar() {
                    let idx = next_local_idx;
                    next_local_idx += 1;
                    MirStorage::Scalar { local_index: idx }
                } else {
                    let size = ty.size_bytes();
                    let align = ty.alignment();
                    let address =
                        memory_layout.allocate(var.name(db), size, align, MirAllocKind::Variable);
                    MirStorage::Memory {
                        address,
                        size,
                        align,
                    }
                };

                locals.push(MirLocal {
                    name: var.name(db),
                    ty,
                    init: None,
                    kind: MirLocalKind::Var,
                    storage,
                });
            }
            other => unreachable!("unexpected variable kind {:?} in monomorphized function", other),
        }
    }

    let return_type = func
        .return_type(db)
        .map(|spec| resolve_any_type(db, spec.infer(db), concrete_spec))
        .transpose()?;

    if let Some(ref ret_ty) = return_type {
        locals.push(MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: MirLocalKind::Var,
            storage: MirStorage::Scalar { local_index: next_local_idx },
        });
        next_local_idx += 1;
    }

    // Lower body statements with ANY type override
    let body = crate::lower::lower_stmt::lower_stmts_with_ctx(db, func.statements(db), Some(concrete_spec), string_pool)?;

    Ok(MirFunction {
        name: mono_name,
        origin_name: func.name(db),
        index,
        params,
        return_type,
        locals,
        body,
        linkage: MirLinkage::Export,
        export_name: None,
    })
}

/// Resolve an already-lowered MIR type, substituting ANY-like elementary types.
fn resolve_any_mir_type(ty: &MirType, concrete: MirElementary) -> MirType {
    match ty {
        // If the type couldn't be lowered (was ANY), use the concrete type
        MirType::Void => MirType::Elementary(concrete),
        _ => ty.clone(),
    }
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
            let non_bool = call.args.iter()
                .filter(|a| a.kind == MirArgKind::ByValue)
                .find_map(|a| infer_concrete_type_from_expr(&a.value))
                .filter(|e| !matches!(e, MirElementary::Bool));
            non_bool.or_else(|| {
                call.args.iter()
                    .filter(|a| a.kind == MirArgKind::ByValue)
                    .find_map(|a| infer_concrete_type_from_expr(&a.value))
            })
        }
    };

    if let Some(concrete) = concrete {
        if let Some(spec) = mir_elementary_to_spec(concrete) {
            out.entry(call.callee).or_default().insert(spec);
        }
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
        // For loads and calls, we'd need type context — but the expression
        // was already typed during lowering. The call's return_type carries it.
        MirExpr::Call(call) => match &call.return_type {
            MirType::Elementary(e) => Some(*e),
            _ => None,
        },
        MirExpr::Load(_, ty) => match ty {
            MirType::Elementary(e) => Some(*e),
            _ => None,
        },
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
        MirElementary::WChar => ElementarySpec::WChar,
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
            let non_bool = call.args.iter()
                .filter(|a| a.kind == MirArgKind::ByValue)
                .find_map(|a| infer_concrete_type_from_expr(&a.value))
                .filter(|e| !matches!(e, MirElementary::Bool));
            non_bool.or_else(|| {
                call.args.iter()
                    .filter(|a| a.kind == MirArgKind::ByValue)
                    .find_map(|a| infer_concrete_type_from_expr(&a.value))
            })
        }
    };

    if let Some(concrete_elem) = concrete {
        if let Some(&(mono_name, new_idx)) = mono_indices.get(&(call.callee, concrete_elem)) {
            call.callee = mono_name;
            call.callee_index = new_idx;
        }
    }
}
