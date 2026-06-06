use db::WorkspaceDataBase;
use hir::{
    Qualifier,
    hir_def::{
        expressions::{expression::InitExprKind, statement::StmtKind},
        interned::identifier::Ident,
        pous::{
            class::Class,
            function::Function,
            function_block::FunctionBlock,
            variable::{VariableDecl, VariableKind},
        },
        program::ProgramDecl,
    },
    hir_ty::{infer::Infer, ty::Type},
};
use rustc_hash::{FxHashMap, FxHashSet};

use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    expr::MirPlace,
    function::{
        MirFunction, MirLinkage, MirLocal, MirLocalKind, MirParam, MirParamKind, MirStorage,
        MirVariableStorage,
    },
    lower::{
        lower_expr::ExprLowerCtx,
        lower_stmt::lower_stmts,
        lower_type::{LowerTypeError, lower_type},
    },
    memory::{MirAllocKind, MirMemoryLayout},
    stmt::MirStmt,
    types::MirType,
};

/// Lower a FUNCTION to a MirFunction.
pub fn lower_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    fb_subs: &FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        FxHashMap<
            hir::hir_def::interned::identifier::Ident,
            hir::hir_def::expressions::spec::ElementarySpec,
        >,
    >,
    fb_mangling: &super::monomorphize::FbInstanceMap<'db>,
) -> Result<MirFunction, LowerTypeError> {
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;

    // Collect address-taken variables for storage decisions
    let address_taken = collect_address_taken_vars(db, func.statements(db));

    // 1. Build parameters (Input, InOut, Output)
    // VAR_OUTPUT is passed as a pointer at the WASM level - the function writes through it.
    //
    // `next_local_idx` advances by the number of wasm-local slots each
    // param actually consumes (see `param_wasm_width`) — STRING params
    // flatten to two i32s, not one, and getting this wrong silently
    // aliases the param's second slot with later Var locals.
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input => {
                let ty = lower_var_type(db, *var)?;
                let param = MirParam {
                    name: var.name(db),
                    ty: ty.clone(),
                    kind: MirParamKind::Input,
                };
                next_local_idx += param_wasm_width(&param.ty, param.kind);
                params.push(param);
            }
            VariableKind::InOut | VariableKind::Output => {
                let ty = lower_var_type(db, *var)?;
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
                next_local_idx += param_wasm_width(&param.ty, param.kind);
                params.push(param);
            }
            _ => {}
        }
    }

    // 2. Return type
    let return_type = func
        .return_type(db)
        .map(|spec| {
            let ty = spec.infer(db);
            lower_type(db, ty)
        })
        .transpose()?;

    // The return slot, named after the function: scalars in a wasm local,
    // STRING and aggregates in linear memory.
    if let Some(ref ret_ty) = return_type {
        let storage = allocate_local_storage(
            func.name(db),
            ret_ty,
            false,
            &mut next_local_idx,
            memory_layout,
        );
        locals.push(MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: MirLocalKind::Var,
            storage,
            // Synthetic return slot in a stateless FUNCTION.
            var_storage: MirVariableStorage::Automatic,
        });
    }

    // 3. Local variables (Var, Temp - Output is a parameter now)
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input | VariableKind::InOut | VariableKind::Output => continue,
            _ => {}
        }

        let ty = lower_var_type_with_mangling(db, *var, fb_subs, fb_mangling)?;
        let storage = allocate_local_storage(
            var.name(db),
            &ty,
            address_taken.contains(&var.name(db)),
            &mut next_local_idx,
            memory_layout,
        );

        let kind = match var.kind(db) {
            VariableKind::Output => MirLocalKind::Output,
            VariableKind::Temp => MirLocalKind::Temp,
            _ => MirLocalKind::Var,
        };

        locals.push(MirLocal {
            name: var.name(db),
            ty,
            init: None, // TODO: lower initializers
            kind,
            storage,
            // FUNCTION locals are stateless — automatic regardless of section.
            var_storage: MirVariableStorage::Automatic,
        });
    }

    // 4. Generate initializer statements for variables with init expressions
    let mut init_stmts = Vec::new();
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input | VariableKind::InOut | VariableKind::Output => continue,
            _ => {}
        }
        if let Some(init_expr) = var.init(db)
            && let Some(stmt) = lower_var_init(db, var.name(db), init_expr)?
        {
            init_stmts.push(stmt);
        }
    }

    // 5. Lower body statements (with FB subs for generic FB instantiation)
    // Build a per-function var-name → mangled-FB-name lookup so FB
    // calls inside the body resolve to the right `__body__` per
    // concrete `T` (e.g. `c_int : Counter<INT>` calls
    // `Counter$INT$__body__`).
    let mut local_fb_mangling: FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        hir::hir_def::interned::identifier::Ident,
    > = FxHashMap::default();
    for var in func.variables(db) {
        if let Some(mangled) = fb_mangling.mangled_for_var(*var) {
            local_fb_mangling.insert(var.name(db), mangled);
        }
    }

    let mut body = if fb_subs.is_empty() && local_fb_mangling.is_empty() {
        lower_stmts(db, func.statements(db), string_pool.clone())?
    } else {
        crate::lower::lower_stmt::lower_stmts_with_fb_subs_and_mangling(
            db,
            func.statements(db),
            fb_subs,
            &local_fb_mangling,
            string_pool.clone(),
        )?
    };
    // Prepend initializers
    if !init_stmts.is_empty() {
        init_stmts.append(&mut body);
        body = init_stmts;
    }
    let body = body;

    // Determine linkage - check if there's an extern pragma
    let is_extern = func
        .statements(db)
        .iter()
        .any(|s| matches!(s.stmt(db), StmtKind::ExternPragma(_)));

    let linkage = if is_extern {
        MirLinkage::Internal // extern functions are imports, handled separately
    } else {
        MirLinkage::Export
    };

    Ok(MirFunction {
        name: super::monomorphize::qualified_pou_ident(db, Type::Function(func)),
        origin_name: func.name(db),
        index,
        params,
        return_type,
        locals,
        body,
        linkage,
        is_test: hir::hir_def::pous::pragma::is_test(db, func.pragmas(db)),
        export_name: None,
    })
}

/// Lower a FUNCTION_BLOCK to MirFunctions (one per method + instance type).
pub fn lower_function_block<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    any_subs: &rustc_hash::FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        hir::hir_def::expressions::spec::ElementarySpec,
    >,
    mangled_name: hir::hir_def::interned::identifier::Ident,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    // Lower each method as a separate function with 'this' parameter
    for method in fb.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter (use substitutions for ANY types)
        let fb_type =
            super::lower_type::lower_fb_type_with_subs_named(db, fb, any_subs, mangled_name)?;
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = lower_var_type(db, *var)?;
                    let param = MirParam {
                        name: var.name(db),
                        ty,
                        kind: MirParamKind::Input,
                    };
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                VariableKind::InOut => {
                    let ty = lower_var_type(db, *var)?;
                    let param = MirParam {
                        name: var.name(db),
                        ty: MirType::Pointer(Box::new(ty)),
                        kind: MirParamKind::InOut,
                    };
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                _ => {
                    let ty = lower_var_type(db, *var)?;
                    let storage = allocate_local_storage(
                        var.name(db),
                        &ty,
                        false,
                        &mut next_local_idx,
                        memory_layout,
                    );
                    locals.push(MirLocal {
                        name: var.name(db),
                        ty,
                        init: None,
                        kind: MirLocalKind::Var,
                        storage,
                        // FB/class method local — stateless per call.
                        var_storage: MirVariableStorage::Automatic,
                    });
                }
            }
        }

        let return_type = method
            .return_type(db)
            .map(|spec| {
                let ty = spec.infer(db);
                lower_type(db, ty)
            })
            .transpose()?;

        // The return slot (scalar → wasm local, else linear memory).
        if let Some(ref ret_ty) = return_type {
            let storage = allocate_local_storage(
                method.name(db),
                ret_ty,
                false,
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                kind: MirLocalKind::Var,
                storage,
                // Synthetic method return slot — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        let body = lower_stmts(db, method.stmts(db), string_pool.clone())?;

        // Qualified name: "FBName$MethodName"
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}${}",
                mangled_name.text(db),
                method.name(db).text(db)
            )),
        );

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            linkage: MirLinkage::Export,
            is_test: false,
            export_name: None,
        });
        idx += 1;
    }

    // Lower FB body as __body__ function
    // All variables (input, output, var) are accessed through the 'this' pointer.
    if !fb.statements(db).is_empty() {
        let fb_type =
            super::lower_type::lower_fb_type_with_subs_named(db, fb, any_subs, mangled_name)?;
        let body_params = vec![MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type.clone())),
            kind: MirParamKind::This,
        }];

        let mut body_locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        let address_taken = collect_address_taken_vars(db, fb.statements(db));

        for var in fb.variables(db) {
            if var.kind(db) == VariableKind::Temp {
                let ty = lower_var_type(db, *var)?;
                let storage = allocate_local_storage(
                    var.name(db),
                    &ty,
                    address_taken.contains(&var.name(db)),
                    &mut next_local_idx,
                    memory_layout,
                );
                body_locals.push(MirLocal {
                    name: var.name(db),
                    ty,
                    init: None,
                    kind: MirLocalKind::Var,
                    storage,
                    // Only VAR_TEMP reaches here (FB persistent state lives in
                    // the instance struct behind `this`).
                    var_storage: MirVariableStorage::Automatic,
                });
            }
        }

        // Body lowering with the `this` struct context.
        let this_struct = match &fb_type {
            MirType::Struct(s) => s.clone(),
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "FB type is not a struct".into(),
                ));
            }
        };
        // Build a global-shaped fb_subs map with just this FB's substitutions
        let fb_subs_map = if any_subs.is_empty() {
            None
        } else {
            let mut map = FxHashMap::default();
            map.insert(fb.name(db), any_subs.clone());
            Some(map)
        };
        // Pick the concrete type from the ANY_* substitutions for expression lowering.
        // All ANY_* variables in the FB resolve to the same concrete type group
        // (via INTO chains), so taking the first value is correct.
        let any_override = any_subs.values().next().copied();
        // Resolve `{#if X is T}` arms against the concrete type before
        // lowering - same dispatch as functions, just for FB bodies.
        let expanded_owned;
        let body_input: &[hir::hir_def::expressions::statement::Stmt<'db>] = match any_override {
            Some(concrete) => {
                expanded_owned = super::monomorphize::expanded_body_for_concrete(
                    db,
                    fb.statements(db),
                    concrete,
                );
                &expanded_owned
            }
            None => fb.statements(db),
        };
        let body_stmts = crate::lower::lower_stmt::lower_stmts_fb_body(
            db,
            body_input,
            this_struct,
            string_pool.clone(),
            fb_subs_map.as_ref(),
            any_override,
        )?;

        let body_name = Ident::new(
            db,
            compact_str::CompactString::from(format!("{}$__body__", mangled_name.text(db))),
        );

        functions.push(MirFunction {
            name: body_name,
            origin_name: fb.name(db),
            index: idx,
            params: body_params,
            return_type: None,
            locals: body_locals,
            body: body_stmts,
            linkage: MirLinkage::Export,
            is_test: false,
            export_name: None,
        });
    }

    Ok(functions)
}

/// Lower a CLASS to MirFunctions (one per method + instance type).
pub fn lower_class<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    for method in class.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter
        let class_type = lower_type(db, Type::Class(class))?;
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(class_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = lower_var_type(db, *var)?;
                    let param = MirParam {
                        name: var.name(db),
                        ty,
                        kind: MirParamKind::Input,
                    };
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                VariableKind::InOut => {
                    let ty = lower_var_type(db, *var)?;
                    let param = MirParam {
                        name: var.name(db),
                        ty: MirType::Pointer(Box::new(ty)),
                        kind: MirParamKind::InOut,
                    };
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                _ => {
                    let ty = lower_var_type(db, *var)?;
                    let storage = allocate_local_storage(
                        var.name(db),
                        &ty,
                        false,
                        &mut next_local_idx,
                        memory_layout,
                    );
                    locals.push(MirLocal {
                        name: var.name(db),
                        ty,
                        init: None,
                        kind: MirLocalKind::Var,
                        storage,
                        // FB/class method local — stateless per call.
                        var_storage: MirVariableStorage::Automatic,
                    });
                }
            }
        }

        let return_type = method
            .return_type(db)
            .map(|spec| {
                let ty = spec.infer(db);
                lower_type(db, ty)
            })
            .transpose()?;

        // Return local: scalar → WASM local, non-scalar → linear memory.
        if let Some(ref ret_ty) = return_type {
            let storage = allocate_local_storage(
                method.name(db),
                ret_ty,
                false,
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                kind: MirLocalKind::Var,
                storage,
                // Synthetic method return slot — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        let body = lower_stmts(db, method.stmts(db), string_pool.clone())?;

        // Qualified name: "<NsPath>.ClassName$MethodName" (or
        // "ClassName$MethodName" for top-level classes).
        let class_qualified = super::monomorphize::qualified_pou_ident(db, Type::Class(class));
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}${}",
                class_qualified.text(db),
                method.name(db).text(db)
            )),
        );

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            linkage: MirLinkage::Export,
            is_test: false,
            export_name: None,
        });
        idx += 1;
    }

    Ok(functions)
}

/// Lower a PROGRAM to a MirFunction (zero-param exported function).
pub fn lower_program<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<MirFunction, LowerTypeError> {
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;

    for var in program.variables(db) {
        let ty = lower_var_type(db, *var)?;
        // PROGRAM keeps state across scans, so its VARs are Static (or
        // Retain/Global per section/qualifier) — unlike FUNCTION locals.
        let var_storage = classify_var_storage(db, *var, /* persists = */ true);
        // A persistent VAR can never be a wasm local (locals reset every
        // scan call), so force it into linear memory even when it is a
        // scalar. Only VAR_TEMP (Automatic) may stay a wasm local.
        let force_memory = var_storage != MirVariableStorage::Automatic;
        let storage = allocate_local_storage(
            var.name(db),
            &ty,
            force_memory,
            &mut next_local_idx,
            memory_layout,
        );
        // RETAIN vars must survive power cycles: register them so Step 2b.2
        // can gather them into a contiguous, host-snapshottable band.
        if var_storage == MirVariableStorage::Retain
            && let MirStorage::Memory {
                address,
                size,
                align,
            } = storage
        {
            memory_layout.record_retain(var.name(db), address, size, align);
        }
        locals.push(MirLocal {
            name: var.name(db),
            ty,
            init: None,
            kind: MirLocalKind::Var,
            var_storage,
            storage,
        });
    }

    let body = lower_stmts(db, program.statements(db), string_pool.clone())?;

    Ok(MirFunction {
        name: super::monomorphize::qualified_pou_ident(db, Type::Program(program)),
        origin_name: program.name(db),
        index,
        params: Vec::new(),
        return_type: None,
        locals,
        body,
        linkage: MirLinkage::Export,
        is_test: false,
        export_name: None,
    })
}

/// Number of wasm i32 locals a parameter of the given (lowered) type
/// and kind consumes when flattened to the wasm function signature.
///
/// This needs to match the layout that `build_local_map` in
/// `wasm_codegen/src/lib.rs` produces, because lowering uses the
/// returned width to advance the wasm-local index counter that
/// downstream Var/return-slot allocation reads. Get this wrong and
/// scalar locals end up assigned to wasm-local indices that alias the
/// STRING param's `(ptr, len)` slots — a silent corruption the wasm
/// validator can't catch (everything is i32). See
/// `crates/wasm_codegen/src/tests/string_audit.rs::known_bug_string_param_clobbered_by_scalar_var`
/// for the regression test.
///
/// Rules:
/// - `VAR_INPUT STRING` → 2 slots `(ptr, len)`
/// - `VAR_IN_OUT STRING` / `VAR_OUTPUT STRING` (`MirType::Pointer(STRING)`)
///    → 2 slots `(addr, cap)`
/// - everything else (scalars, pointers to scalars, struct refs) → 1 slot
pub fn param_wasm_width(ty: &MirType, kind: MirParamKind) -> u32 {
    match (kind, ty) {
        (MirParamKind::Input, MirType::String { .. }) => 2,
        (MirParamKind::InOut | MirParamKind::Output, MirType::Pointer(inner))
            if matches!(inner.as_ref(), MirType::String { .. }) =>
        {
            2
        }
        _ => 1,
    }
}

/// Allocate storage for a variable, picking a wasm-local (only for a
/// scalar that may be kept in a register) or a linear-memory address
/// (everything else, including STRING and any composite type).
///
/// `force_memory` makes a scalar live in linear memory even though its
/// type would otherwise fit a wasm local. Two situations require it:
/// - the scalar is **address-taken** (`REF(x)`), so it needs an address;
/// - the scalar is **persistent** (a PROGRAM/FB-instance `VAR`,
///   `var_storage != Automatic`): wasm locals are reset on every call, so
///   anything that must survive across scan cycles cannot be a local. Only
///   `Automatic` storage (function locals, `VAR_TEMP`) may use a wasm local.
///
/// The shared form of what used to be open-coded in every MIR
/// lowering path — `lower_function`, `lower_function_block`,
/// `lower_wasm_intrinsic`, and the various branches in `monomorphize`.
/// Each had its own variant; three of them got the type-based
/// decision wrong for STRING returns at some point in the past, which
/// is why this lives in one place now.
pub fn allocate_local_storage(
    name: Ident,
    ty: &MirType,
    force_memory: bool,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) -> MirStorage {
    if ty.is_scalar() && !force_memory {
        let idx = *next_local_idx;
        *next_local_idx += 1;
        MirStorage::Scalar { local_index: idx }
    } else {
        let size = ty.size_bytes();
        let align = ty.alignment();
        let address = memory_layout.allocate(name, size, align, MirAllocKind::Variable);
        MirStorage::Memory {
            address,
            size,
            align,
        }
    }
}

/// Classify a variable's storage *duration* (lifetime) for the runtime.
///
/// `persists` is true when the containing POU keeps state across scans — a
/// `PROGRAM` or a `FUNCTION_BLOCK` instance. A `FUNCTION` is stateless, so
/// all of its locals are [`MirVariableStorage::Automatic`] regardless of
/// section or qualifier.
///
/// Note this reads `var.kind` directly, so `VAR_TEMP` is `Automatic` even
/// inside a persistent POU, and `VAR_GLOBAL` is `Global` regardless of
/// `persists`.
fn classify_var_storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    persists: bool,
) -> MirVariableStorage {
    match var.kind(db) {
        VariableKind::Temp => MirVariableStorage::Automatic,
        VariableKind::Global => MirVariableStorage::Global,
        _ if !persists => MirVariableStorage::Automatic,
        _ if var.qualifier(db).contains(Qualifier::RETAIN) => MirVariableStorage::Retain,
        _ => MirVariableStorage::Static,
    }
}

/// Lower a variable's type spec, recovering a declared `STRING[N]`
/// capacity that `Type::normalize` collapses.
fn lower_var_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    use hir::hir_def::expressions::spec::SpecKind;

    let ty = var.spec(db).infer(db);
    let mir = lower_type(db, ty)?;

    // For STRING types, override the default capacity with the declared
    // `[N]` if the spec carries one.
    if matches!(mir, MirType::String { .. })
        && let SpecKind::SizedString(length_expr) = var.spec(db).kind(db)
        && let Some(n) = length_expr.as_range(db)
    {
        return Ok(MirType::String { capacity: n as u32 });
    }
    Ok(mir)
}

/// Lower a variable's type, resolving FB types using the per-variable
/// FB instantiation map (preferred), with fallback to the legacy
/// global FB-name → subs map.
fn lower_var_type_with_mangling<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    fb_subs: &FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        FxHashMap<
            hir::hir_def::interned::identifier::Ident,
            hir::hir_def::expressions::spec::ElementarySpec,
        >,
    >,
    fb_mangling: &super::monomorphize::FbInstanceMap<'db>,
) -> Result<MirType, LowerTypeError> {
    let ty = var.spec(db).infer(db);
    let Type::FunctionBlock(fb) = ty else {
        // Non-FB types: use the per-variable lowering so spec-level
        // overrides (e.g. `STRING[N]` capacity) survive.
        return lower_var_type(db, var);
    };
    // Prefer per-variable mangled instantiation: this picks the correct
    // struct name when two variables of the same FB use distinct
    // concrete types (e.g. `Counter<INT>` and `Counter<DINT>`).
    if let Some(mangled) = fb_mangling.mangled_for_var(var) {
        let subs = fb_mangling
            .subs_for_mangled(mangled)
            .cloned()
            .unwrap_or_default();
        return super::lower_type::lower_fb_type_with_subs_named(db, fb, &subs, mangled);
    }
    // Fallback: non-generic FB or unrecognized variable — keep the
    // legacy subs lookup so existing single-instantiation behavior is
    // preserved.
    if let Some(subs) = fb_subs.get(&fb.name(db)) {
        super::lower_type::lower_fb_type_with_subs(db, fb, subs)
    } else {
        lower_type(db, ty)
    }
}

/// Collect identifiers of variables whose address is taken (via REF()).
/// These must be allocated in linear memory even if they're scalars.
fn collect_address_taken_vars<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
) -> FxHashSet<Ident> {
    use hir::hir_def::expressions::expression::{ExprKind, PrimaryExpr, RefValue};

    let mut result = FxHashSet::default();

    fn walk_expr<'db>(
        db: &'db dyn WorkspaceDataBase,
        expr: hir::hir_def::expressions::expression::Expr<'db>,
        result: &mut FxHashSet<Ident>,
    ) {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(begin_path),
            }) => {
                // REF(var) - extract the variable name
                if let Some(path_expr) = begin_path.expr(db) {
                    let ident = path_expr.ident(db).ident;
                    result.insert(ident);
                }
            }
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_)) => {}
            ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(fc)) => {
                for param in fc.params(db) {
                    match param.kind(db) {
                        hir::hir_def::expressions::expression::ParamAssignKind::NonFormal {
                            value,
                        }
                        | hir::hir_def::expressions::expression::ParamAssignKind::FormalInput {
                            value,
                            ..
                        } => {
                            walk_expr(db, value, result);
                        }
                        hir::hir_def::expressions::expression::ParamAssignKind::FormalOutput {
                            variable,
                            ..
                        } => {
                            // OUT => x takes the address of x
                            if let hir::hir_def::expressions::expression::VariableAccessKind::Symbolic(begin_path) = &variable.kind(db)
                                && let Some(path_expr) = begin_path.expr(db) {
                                    result.insert(path_expr.ident(db).ident);
                                }
                        }
                    }
                }
            }
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => {
                walk_expr(db, *inner, result);
            }
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right } => {
                walk_expr(db, *left, result);
                walk_expr(db, *right, result);
            }
            ExprKind::UnaryOperator { expr: inner, .. } => {
                walk_expr(db, *inner, result);
            }
            _ => {}
        }
    }

    fn walk_stmts<'db>(
        db: &'db dyn WorkspaceDataBase,
        stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
        result: &mut FxHashSet<Ident>,
    ) {
        use hir::hir_def::expressions::statement::StmtKind;
        for stmt in stmts {
            if let StmtKind::Assignment { target: _, var: _ } = stmt.stmt(db) {
                // `var` is the target and `target` the value expression, as HIR
                // names them.
            }
            // Walk all expressions in the statement
            walk_stmt_exprs(db, *stmt, result);
        }
    }

    fn walk_stmt_exprs<'db>(
        db: &'db dyn WorkspaceDataBase,
        stmt: hir::hir_def::expressions::statement::Stmt<'db>,
        result: &mut FxHashSet<Ident>,
    ) {
        use hir::hir_def::expressions::statement::StmtKind;
        match stmt.stmt(db) {
            StmtKind::Assignment { var: _, target } => {
                walk_expr(db, *target, result);
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                walk_expr(db, *condition, result);
                if let Some(stmts) = then {
                    walk_stmts(db, stmts, result);
                }
                for (cond, body) in else_if {
                    walk_expr(db, *cond, result);
                    walk_stmts(db, body, result);
                }
                if let Some(stmts) = else_ {
                    walk_stmts(db, stmts, result);
                }
            }
            StmtKind::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                walk_expr(db, *start, result);
                walk_expr(db, *end, result);
                if let Some(s) = step {
                    walk_expr(db, *s, result);
                }
                walk_stmts(db, body, result);
            }
            StmtKind::While { condition, body } => {
                walk_expr(db, *condition, result);
                walk_stmts(db, body, result);
            }
            StmtKind::Repeat { condition, body } => {
                walk_expr(db, *condition, result);
                walk_stmts(db, body, result);
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                walk_expr(db, *condition, result);
                for (_, body) in cases {
                    walk_stmts(db, body, result);
                }
                if let Some(stmts) = else_ {
                    walk_stmts(db, stmts, result);
                }
            }
            StmtKind::FuncCall(fc) => {
                for param in fc.params(db) {
                    match param.kind(db) {
                        hir::hir_def::expressions::expression::ParamAssignKind::NonFormal { value }
                        | hir::hir_def::expressions::expression::ParamAssignKind::FormalInput { value, .. } => {
                            walk_expr(db, value, result);
                        }
                        hir::hir_def::expressions::expression::ParamAssignKind::FormalOutput { variable, .. } => {
                            if let hir::hir_def::expressions::expression::VariableAccessKind::Symbolic(begin_path) = &variable.kind(db)
                                && let Some(path_expr) = begin_path.expr(db) {
                                    result.insert(path_expr.ident(db).ident);
                                }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    walk_stmts(db, stmts, &mut result);
    result
}

/// Lower a variable initializer to an assignment statement.
/// Returns None if the init expression can't be lowered (e.g., complex struct inits).
fn lower_var_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
    init_expr: hir::hir_def::expressions::expression::InitExpr<'db>,
) -> Result<Option<MirStmt>, LowerTypeError> {
    match init_expr.kind(db) {
        InitExprKind::ConstantExpr(expr) => {
            let ctx = ExprLowerCtx::new(
                db,
                Rc::new(RefCell::new(super::lower_expr::StringPool::default())),
            );
            let value = ctx.lower_expr(expr)?;
            Ok(Some(MirStmt::Assign {
                target: MirPlace::Local(var_name),
                value,
            }))
        }
        // TODO: ArrayInit, StructInit
        _ => Ok(None),
    }
}
