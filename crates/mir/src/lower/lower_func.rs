use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::statement::StmtKind,
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
    // Phase B: for a specialized copy, each interface param's concrete
    // implementer.
    iface_subs: Option<
        &FxHashMap<hir::hir_def::interned::identifier::Ident, hir::hir_def::pous::pou::Pou<'db>>,
    >,
    // Phase B: module-global call-site -> mangled specialization rewrites, so
    // calls in this body route to the right specialization.
    iface_call_rewrites: &FxHashMap<
        hir::hir_def::expressions::expression::FuncCall<'db>,
        hir::hir_def::interned::identifier::Ident,
    >,
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
        // Phase B: an interface VAR_IN_OUT param is specialized to a pointer to
        // the concrete implementer's instance struct — the same calling
        // convention as a normal InOut FB pointer, so the arg (`&aWorker`)
        // becomes `dev`, and `dev.Method()`'s `this` falls out for free.
        if let Some(concrete) = iface_subs.and_then(|m| m.get(&var.name(db))) {
            let ty = lower_type(db, hir::hir_ty::ty::Type::new_pou(db, *concrete))?;
            let param = MirParam {
                name: var.name(db),
                ty: MirType::Pointer(Box::new(ty)),
                kind: MirParamKind::InOut,
            };
            next_local_idx += param_wasm_width(&param.ty, param.kind);
            params.push(param);
            continue;
        }
        match var.kind(db) {
            VariableKind::Input => {
                let ty = input_param_type(lower_var_type(db, *var)?);
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
            // VAR_EXTERNAL resolves to a global's address, not a function local.
            VariableKind::Input
            | VariableKind::InOut
            | VariableKind::Output
            | VariableKind::External => continue,
            _ => {}
        }

        let ty = lower_var_type(db, *var)?;
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
        if let Some(init_expr) = var.init(db) {
            let var_ty = lower_var_type(db, *var)?;
            lower_var_init(
                db,
                var.name(db),
                &var_ty,
                init_expr,
                &string_pool,
                &mut init_stmts,
            )?;
        }
    }

    // 5. Body statements. Interface specialization threads `iface_subs` and
    // `iface_call_rewrites` into the body.
    let needs_full_ctx =
        iface_subs.is_some_and(|m| !m.is_empty()) || !iface_call_rewrites.is_empty();
    let (mut body, call_scratch) = if !needs_full_ctx {
        lower_stmts(db, func.statements(db), string_pool.clone())?
    } else {
        crate::lower::lower_stmt::lower_stmts_with_ctx(
            db,
            func.statements(db),
            iface_subs,
            iface_call_rewrites,
            string_pool.clone(),
        )?
    };
    append_call_scratch_locals(
        call_scratch,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );
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
        name: super::naming::mir_function_symbol(db, func),
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
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    // The FB's qualified symbol: the instance struct name and every method
    // symbol derive from it.
    let fb_qualified = super::naming::qualified_pou_ident(db, Type::FunctionBlock(fb));

    // Lower each method as a separate function with 'this' parameter
    for method in fb.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter — the FB's instance struct.
        let fb_type = super::lower_type::lower_fb_type(db, fb)?;
        // The method body resolves bare member access (implicit THIS) against
        // this struct.
        let this_struct = match &fb_type {
            MirType::Struct(s) => s.clone(),
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "FB type is not a struct".into(),
                ));
            }
        };
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = input_param_type(lower_var_type(db, *var)?);
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

        let (body, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
            db,
            method.stmts(db),
            this_struct,
            string_pool.clone(),
            iface_call_rewrites,
        )?;
        append_call_scratch_locals(
            call_scratch,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );

        // Method symbol: `<FB>#<method>`. The `#` separator is distinct from `.`
        // (namespace) so the symbol is unambiguously parseable — e.g.
        // `NsA.Counter#inc` is FB `NsA.Counter`, method `inc`.
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}#{}",
                fb_qualified.text(db),
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
        let fb_type = super::lower_type::lower_fb_type(db, fb)?;
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
        let (body_stmts, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
            db,
            fb.statements(db),
            this_struct,
            string_pool.clone(),
            iface_call_rewrites,
        )?;
        append_call_scratch_locals(
            call_scratch,
            &mut body_locals,
            &mut next_local_idx,
            memory_layout,
        );

        let body_name = Ident::new(
            db,
            compact_str::CompactString::from(format!("{}$__body__", fb_qualified.text(db))),
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
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    for method in class.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter
        let class_type = lower_type(db, Type::Class(class))?;
        // The method body resolves bare member access (implicit THIS) against
        // this struct.
        let this_struct = match &class_type {
            MirType::Struct(s) => s.clone(),
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "class type is not a struct".into(),
                ));
            }
        };
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(class_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = input_param_type(lower_var_type(db, *var)?);
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

        let (body, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
            db,
            method.stmts(db),
            this_struct,
            string_pool.clone(),
            iface_call_rewrites,
        )?;
        append_call_scratch_locals(
            call_scratch,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );

        // Method symbol: `<NsPath.>Class#Method` (see the FB-method site).
        let class_qualified = super::naming::qualified_pou_ident(db, Type::Class(class));
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}#{}",
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

/// Lower a PROGRAM's body to `<Prog>$__body__(this)` plus its instance
/// struct: a PROGRAM is compiled like a FUNCTION_BLOCK, with only
/// `VAR_TEMP` body-local. Instances are allocated per program
/// configuration in `lower_module`.
pub fn lower_program<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<(MirFunction, MirType), LowerTypeError> {
    let prog_type = super::lower_type::lower_program_type(db, program)?;
    let this_struct = match &prog_type {
        MirType::Struct(s) => s.clone(),
        _ => {
            return Err(LowerTypeError::UnsupportedType(
                "program type is not a struct".into(),
            ));
        }
    };

    let params = vec![MirParam {
        name: Ident::new(db, compact_str::CompactString::from("this")),
        ty: MirType::Pointer(Box::new(prog_type.clone())),
        kind: MirParamKind::This,
    }];

    // VAR_TEMP become body locals (automatic); persistent vars are instance
    // fields accessed through `this`.
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 1; // 0 is 'this'
    let address_taken = collect_address_taken_vars(db, program.statements(db));
    for var in program.variables(db) {
        if var.kind(db) == VariableKind::Temp {
            let ty = lower_var_type(db, *var)?;
            let storage = allocate_local_storage(
                var.name(db),
                &ty,
                address_taken.contains(&var.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: var.name(db),
                ty,
                init: None,
                kind: MirLocalKind::Var,
                storage,
                var_storage: MirVariableStorage::Automatic,
            });
        }
    }

    let (body, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
        db,
        program.statements(db),
        this_struct,
        string_pool.clone(),
        iface_call_rewrites,
    )?;
    append_call_scratch_locals(
        call_scratch,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );

    let body_name = Ident::new(
        db,
        compact_str::CompactString::from(format!("{}$__body__", program.name(db).text(db))),
    );

    let func = MirFunction {
        name: body_name,
        origin_name: program.name(db),
        index,
        params,
        return_type: None,
        locals,
        body,
        linkage: MirLinkage::Export,
        is_test: false,
        export_name: None,
    };
    Ok((func, prog_type))
}

/// Number of wasm i32 locals a parameter consumes in the signature; must
/// match `build_local_map` in `wasm_codegen`, since lowering advances the
/// wasm-local index by it. `VAR_INPUT STRING` → `(ptr, len)`, a STRING
/// pointer → `(addr, cap)`, everything else one slot.
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
/// Aggregate (struct/array) `VAR_INPUT`s are received as a POINTER to the
/// caller's call-entry snapshot (the caller copies the arg into a scratch
/// local and passes its address — `MirExpr::CopyIntoScratch` — mirroring the
/// FB input copy-in). Everything else stays a value param.
pub(crate) fn input_param_type(ty: MirType) -> MirType {
    match ty {
        MirType::Struct(_) | MirType::Array(_) => MirType::Pointer(Box::new(ty)),
        other => other,
    }
}

/// Drain the scratch locals a body's lowering synthesized into the
/// function's locals; memory-forced, since the call site takes their
/// address.
pub(crate) fn append_call_scratch_locals(
    scratch: crate::lower::lower_expr::CallScratch,
    locals: &mut Vec<MirLocal>,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) {
    for (name, ty) in scratch {
        let storage = allocate_local_storage(name, &ty, true, next_local_idx, memory_layout);
        locals.push(MirLocal {
            name,
            ty,
            init: None,
            kind: MirLocalKind::Temp,
            storage,
            var_storage: MirVariableStorage::Automatic,
        });
    }
}

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

/// Lower a variable's type spec, recovering a declared `STRING[N]`
/// capacity that `Type::normalize` collapses.
pub(crate) fn lower_var_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    let ty = var.spec(db).infer(db);
    let mir = lower_type(db, ty)?;
    // Honor a declared `STRING[N]` capacity (shared with field/global lowering).
    Ok(super::lower_type::apply_sized_string(db, var.spec(db), mir))
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
                // Inout args take the variable's address too (see the
                // statement-position FuncCall arm).
                mark_inout_call_args(db, *fc, result);
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
                // A VAR_IN_OUT arg is passed by reference on every callable, so a
                // scalar arg must live in linear memory.
                mark_inout_call_args(db, *fc, result);
            }
            _ => {}
        }
    }

    walk_stmts(db, stmts, &mut result);
    result
}

/// Mark the root variable of every VAR_IN_OUT argument as address-taken,
/// so `&arg` has a target; matters for scalar locals.
fn mark_inout_call_args<'db>(
    db: &'db dyn WorkspaceDataBase,
    fc: hir::hir_def::expressions::expression::FuncCall<'db>,
    result: &mut FxHashSet<Ident>,
) {
    use hir::hir_def::expressions::expression::{
        ExprKind, ParamAssignKind, PrimaryExpr, VariableAccessKind,
    };

    let path = fc.path(db);
    let body = hir::hir_ty::body::infer_body(db, path.scope_id(db));
    for param in fc.params(db) {
        let Some(var) = body.variable_of_param.get(param) else {
            continue;
        };
        if !var.is_in_out(db) {
            continue;
        }
        let value = match param.kind(db) {
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                value
            }
            ParamAssignKind::FormalOutput { .. } => continue,
        };
        if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = value.expr(db)
            && let VariableAccessKind::Symbolic(begin_path) = va.kind(db)
            && let Some(path_expr) = begin_path.expr(db)
        {
            result.insert(path_expr.ident(db).ident);
        }
    }
}

/// Lower a FUNCTION-local variable initializer to prepended assignment(s). A
/// FUNCTION is stateless, so its locals are re-initialized on every call — these
/// statements run at the top of the body. Consumes the HIR's authoritative
/// resolved leaves (the same source as the stateful `__init` path), so aggregate
/// inits (arrays, structs, multi-dim, repetition) lower correctly instead of
/// being dropped. Each leaf targets the local at its path offset.
fn lower_var_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
    var_ty: &crate::types::MirType,
    init_expr: hir::hir_def::expressions::expression::InitExpr<'db>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
    out: &mut Vec<MirStmt>,
) -> Result<(), LowerTypeError> {
    use crate::expr::MirPlace;
    use hir::hir_ty::head::init_inference::infer_initialization;

    let scope = init_expr.scope_id(db);
    let inference = infer_initialization(db, scope);
    let Some(leaves) = inference.init_expr_result.resolved.get(&init_expr) else {
        return Ok(());
    };
    for leaf in leaves {
        let value = ExprLowerCtx::new(db, string_pool.clone()).lower_expr(leaf.value)?;
        let target = if leaf.path.is_empty() {
            MirPlace::Local(var_name)
        } else {
            let Some((offset, leaf_ty)) = walk_init_path(var_ty, &leaf.path) else {
                continue;
            };
            // `field_name` is metadata only — addressing uses `field_offset` over
            // the local's base address.
            MirPlace::Field {
                base: Box::new(MirPlace::Local(var_name)),
                field_name: var_name,
                field_offset: offset,
                field_type: leaf_ty,
            }
        };
        out.push(MirStmt::Assign { target, value });
    }
    Ok(())
}

/// Emit one `Assign { Global, value }` per resolved initializer leaf; MIR
/// walks each leaf's path over the layout and never re-walks the
/// `InitExpr` tree.
pub(crate) fn lower_resolved_init_into<'db>(
    db: &'db dyn WorkspaceDataBase,
    base: u32,
    ty: &crate::types::MirType,
    init: hir::hir_def::expressions::expression::InitExpr<'db>,
    out: &mut Vec<crate::stmt::MirStmt>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<(), LowerTypeError> {
    use crate::expr::MirPlace;
    use crate::stmt::MirStmt;
    use hir::hir_ty::head::init_inference::infer_initialization;

    let scope = init.scope_id(db);
    let inference = infer_initialization(db, scope);
    let Some(leaves) = inference.init_expr_result.resolved.get(&init) else {
        return Ok(()); // HIR produced no resolved leaves (non-flattenable init)
    };
    for leaf in leaves {
        let value = ExprLowerCtx::new(db, string_pool.clone()).lower_expr(leaf.value)?;
        if !is_const_value(&value) {
            continue; // non-const init element — validated/diagnosed by the HIR
        }
        let Some((offset, leaf_ty)) = walk_init_path(ty, &leaf.path) else {
            continue;
        };
        out.push(MirStmt::Assign {
            target: MirPlace::Global {
                address: base + offset,
                ty: leaf_ty,
            },
            value,
        });
    }
    Ok(())
}

/// Walk a resolved leaf's path over the layout to its byte offset and
/// type.
fn walk_init_path(
    root: &crate::types::MirType,
    path: &[hir::hir_ty::head::init_inference::InitPathStep],
) -> Option<(u32, crate::types::MirType)> {
    use crate::types::MirType;
    use hir::hir_ty::head::init_inference::InitPathStep;
    let mut offset = 0u32;
    let mut cur = root;
    for step in path {
        match (cur, step) {
            (MirType::Struct(s), InitPathStep::Field(name)) => {
                let f = s.fields.iter().find(|f| f.name == *name)?;
                offset += f.offset;
                cur = &f.ty;
            }
            (MirType::Array(a), InitPathStep::ArrayElem(idx)) => {
                offset += *idx * a.element_size;
                cur = a.element_type.as_ref();
            }
            _ => return None,
        }
    }
    Some((offset, cur.clone()))
}

/// A value that can be baked into `__init`: a scalar literal, a string literal,
/// or arithmetic over literals (computed once at startup). Excludes variable
/// loads (init-order hazards) and calls. A `StringLiteral` is allowed because
/// its assignment routes through `rk.str_assign`, a bounded copy into the
/// destination's inline buffer.
fn is_const_value(e: &crate::expr::MirExpr) -> bool {
    use crate::expr::MirExpr;
    match e {
        MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => true,
        MirExpr::BinOp { lhs, rhs, .. } => is_const_value(lhs) && is_const_value(rhs),
        MirExpr::UnaryOp { expr, .. } => is_const_value(expr),
        MirExpr::Cast { expr, .. } => is_const_value(expr),
        MirExpr::Load(..)
        | MirExpr::Call(_)
        | MirExpr::AddrOf(_)
        | MirExpr::CopyIntoScratch { .. } => false,
    }
}
