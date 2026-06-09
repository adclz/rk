use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::{
    expressions::statement::StmtKind,
    pous::{function::Function, pou::Pou, variable::VariableKind},
    semantic_index::SemanticIndex,
};
use hir::hir_ty::infer::Infer;
use rustc_hash::FxHashMap;

use crate::{
    MirInstanceField, MirInstanceType, MirModule,
    function::{MirExternFunction, MirParam, MirParamKind},
    lower::{
        lower_func::{lower_class, lower_function, lower_function_block, lower_program},
        lower_type::{LowerTypeError, lower_type},
        monomorphize::{AnyFunctionInfo, detect_any_function, monomorphize},
    },
    memory::MirMemoryLayout,
    types::MirType,
};

/// Lower multiple HIR semantic indices (from multiple files) into a single MirModule.
pub fn lower_modules<'db>(
    db: &'db dyn WorkspaceDataBase,
    indices: &[&'db SemanticIndex<'db>],
) -> Result<MirModule, LowerTypeError> {
    // Every POU and program, each with its optional namespace prefix.
    let mut all_pous: Vec<(&Pou<'db>, Option<String>)> = Vec::new();
    let mut all_programs: Vec<(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)> =
        Vec::new();
    let mut all_configs: Vec<hir::hir_def::config::ConfigDecl<'db>> = Vec::new();
    for index in indices {
        all_pous.extend(index.global_pous.iter().map(|p| (p, None)));
        all_programs.extend(index.programs.iter().map(|p| (p, None)));
        all_configs.extend(index.configs.iter().copied());
        for ns in index.namespaces.iter() {
            collect_namespace_pous(db, ns, &mut all_pous);
        }
    }
    lower_module_from_pous(db, &all_pous, &all_programs, &all_configs)
}

/// Lower a complete HIR semantic index into a MirModule.
pub fn lower_module<'db>(
    db: &'db dyn WorkspaceDataBase,
    index: &SemanticIndex<'db>,
) -> Result<MirModule, LowerTypeError> {
    lower_module_from_pous(
        db,
        &index
            .global_pous
            .iter()
            .map(|p| (p, None))
            .collect::<Vec<_>>(),
        &index.programs.iter().map(|p| (p, None)).collect::<Vec<_>>(),
        &index.configs.iter().copied().collect::<Vec<_>>(),
    )
}

fn lower_module_from_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
    all_configs: &[hir::hir_def::config::ConfigDecl<'db>],
) -> Result<MirModule, LowerTypeError> {
    let mut functions = Vec::new();
    let mut extern_functions = Vec::new();
    let mut function_indices = FxHashMap::default();
    let type_indices = FxHashMap::default();
    let mut instance_types = Vec::new();
    let mut memory_layout = MirMemoryLayout::new();
    let string_pool = std::rc::Rc::new(std::cell::RefCell::new(
        super::lower_expr::StringPool::new(0), // base offset set later after memory layout is finalized
    ));
    let mut next_fn_idx: u32 = 0;

    // Collect every unique FB instantiation actually used at call
    // sites. `var_to_mangled_fb` maps each FB-instance variable to its
    // mangled FB name so consumers can look up the right type.
    let (fb_instances, var_to_mangled_fb) =
        super::monomorphize::collect_fb_instantiations(db, all_pous);
    // Group instantiations by FB *identity* (not bare name) so two
    // same-named FBs in different namespaces don't share a bucket.
    let mut fb_instances_by_fb: FxHashMap<
        hir::hir_def::pous::function_block::FunctionBlock<'db>,
        Vec<&super::monomorphize::FbInstance<'db>>,
    > = FxHashMap::default();
    for inst in &fb_instances {
        fb_instances_by_fb.entry(inst.fb).or_default().push(inst);
    }
    // Per-variable mangling lookup used by consumers
    // (function-local variable typing, FB calls).
    let fb_mangling =
        super::monomorphize::FbInstanceMap::from_instances(fb_instances.clone(), var_to_mangled_fb);
    // Backward-compat: bridge to the old `all_fb_subs` shape consumed
    // by `lower_function` / `ExprLowerCtx.fb_subs`. Last-write-wins per
    // FB name (same as the pre-refactor behavior). The per-variable
    // path will replace this shortly.
    let mut all_fb_subs: FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        FxHashMap<
            hir::hir_def::interned::identifier::Ident,
            hir::hir_def::expressions::spec::ElementarySpec,
        >,
    > = FxHashMap::default();
    for inst in &fb_instances {
        if !inst.subs.is_empty() {
            all_fb_subs.insert(inst.fb.name(db), inst.subs.clone());
        }
    }

    // Collect ANY_* functions for deferred monomorphization
    let mut any_functions: Vec<AnyFunctionInfo<'db>> = Vec::new();

    // Collect test entries for the manifest
    let mut test_entries: Vec<crate::test_manifest::TestEntry> = Vec::new();

    // Helper: build a qualified export name from a namespace prefix and bare name.
    let make_export_name = |ns_prefix: &Option<String>, bare_name: &str| -> Option<CompactString> {
        ns_prefix
            .as_ref()
            .map(|prefix| CompactString::from(format!("{}.{}", prefix, bare_name)))
    };

    // Phase 1: Process imports first (extern functions get lower indices)
    for (pou, _ns_prefix) in all_pous.iter() {
        if let Pou::Function(func) = pou {
            let extern_decl = find_extern_decl(db, *func);
            if extern_decl.is_none() {
                continue;
            }
            let extern_decl = extern_decl.unwrap();

            // Check if ANY_* - defer to monomorphization
            if let Some(any_info) = detect_any_function(db, *func) {
                any_functions.push(any_info);
                continue;
            }

            // Lower non-ANY extern function to MirExternFunction
            let mir_ext = lower_extern_function(db, *func, &extern_decl, next_fn_idx)?;
            function_indices.insert(mir_ext.name, next_fn_idx);
            next_fn_idx += 1;
            extern_functions.push(mir_ext);
        }
    }

    // Phase 2: Process local functions
    for (pou, ns_prefix) in all_pous.iter() {
        match pou {
            Pou::Function(func) => {
                let func_id = super::monomorphize::qualified_pou_ident(
                    db,
                    hir::hir_ty::ty::Type::Function(*func),
                );
                // Skip already-processed externs and ANY_* functions
                if function_indices.contains_key(&func_id) {
                    continue;
                }
                if any_functions.iter().any(|a| a.func == *func) {
                    continue;
                }

                // Check if extern (already handled in phase 1)
                let is_extern = func
                    .statements(db)
                    .iter()
                    .any(|s| matches!(s.stmt(db), StmtKind::ExternPragma(_)));
                if is_extern {
                    continue;
                }

                // Check if non-extern ANY_* (deferred) - must check before wasm intrinsic
                // so that ANY_* wasm functions go through monomorphization
                if let Some(any_info) = detect_any_function(db, *func) {
                    any_functions.push(any_info);
                    continue;
                }

                // Skip functions with ANY-typed variables that weren't caught above
                let has_any_var = func.variables(db).iter().any(|v| {
                    let ty = v.spec(db).infer(db).normalize(db);
                    matches!(ty, hir::hir_ty::ty::Type::Elementary(e) if e.is_any())
                });
                if has_any_var {
                    continue;
                }

                // Check if wasm intrinsic (non-ANY) - lower as inline function
                let wasm_decl = func.statements(db).iter().find_map(|s| {
                    if let StmtKind::WasmPragma(decl) = s.stmt(db) {
                        Some(decl.clone())
                    } else {
                        None
                    }
                });
                if let Some(wasm_decl) = wasm_decl {
                    if let Ok(mut mir_func) =
                        lower_wasm_intrinsic(db, *func, &wasm_decl, next_fn_idx, &mut memory_layout)
                    {
                        mir_func.export_name = make_export_name(ns_prefix, func.name(db).text(db));

                        if hir::hir_def::pous::pragma::is_test(db, func.pragmas(db)) {
                            let export_name = mir_func
                                .export_name
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| mir_func.name.text(db).to_string());
                            let cases = build_test_cases(db, *func, &export_name);
                            test_entries.push(crate::test_manifest::TestEntry {
                                path: export_name.clone(),
                                export: export_name,
                                cases,
                            });
                        }

                        function_indices.insert(mir_func.name, next_fn_idx);
                        next_fn_idx += 1;
                        functions.push(mir_func);
                    }
                    continue;
                }

                // An empty body is a valid no-op stub and still lowers, so call sites
                // resolve.
                let mut mir_func = lower_function(
                    db,
                    *func,
                    next_fn_idx,
                    &mut memory_layout,
                    string_pool.clone(),
                    &all_fb_subs,
                    &fb_mangling,
                )?;
                mir_func.export_name = make_export_name(ns_prefix, func.name(db).text(db));
                function_indices.insert(mir_func.name, next_fn_idx);
                next_fn_idx += 1;

                // Collect test entry if marked with {test}
                if hir::hir_def::pous::pragma::is_test(db, func.pragmas(db)) {
                    let export_name = mir_func
                        .export_name
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| mir_func.name.text(db).to_string());
                    let cases = build_test_cases(db, *func, &export_name);
                    test_entries.push(crate::test_manifest::TestEntry {
                        path: export_name.clone(),
                        export: export_name,
                        cases,
                    });
                }

                functions.push(mir_func);
            }

            Pou::FunctionBlock(fb) => {
                // Emit one set of (instance type + body + methods) per
                // unique `(FB, T)` instantiation. Generic FBs that were
                // never instantiated produce no entries in the
                // collector; skip them here too.
                let Some(insts) = fb_instances_by_fb.get(fb) else {
                    continue;
                };

                for inst in insts {
                    let any_subs = inst.subs.clone();
                    let mangled = inst.mangled_name;

                    // Instance type with the mangled struct name.
                    let fb_mir_type = super::lower_type::lower_fb_type_with_subs_named(
                        db, *fb, &any_subs, mangled,
                    )?;
                    if let MirType::Struct(ref struct_type) = fb_mir_type {
                        let inst_fields: Vec<MirInstanceField> = struct_type
                            .fields
                            .iter()
                            .map(|f| {
                                let nested = match &f.ty {
                                    MirType::Struct(inner) => Some(inner.name),
                                    _ => None,
                                };
                                MirInstanceField {
                                    name: f.name,
                                    ty: f.ty.clone(),
                                    offset: f.offset,
                                    nested_instance: nested,
                                    init: None, // TODO: compute initializers
                                }
                            })
                            .collect();

                        instance_types.push(MirInstanceType {
                            name: mangled,
                            fields: inst_fields,
                            size: struct_type.size,
                            align: struct_type.align,
                        });
                    }

                    // Lower methods + __body__ with the mangled name.
                    let method_funcs = lower_function_block(
                        db,
                        *fb,
                        next_fn_idx,
                        &mut memory_layout,
                        string_pool.clone(),
                        &any_subs,
                        mangled,
                    )?;
                    for mf in method_funcs {
                        function_indices.insert(mf.name, mf.index);
                        next_fn_idx += 1;
                        functions.push(mf);
                    }
                }
            }

            Pou::Class(class) => {
                // Build instance type
                let class_type = lower_type(db, hir::hir_ty::ty::Type::Class(*class))?;
                if let MirType::Struct(ref struct_type) = class_type {
                    let inst_fields: Vec<MirInstanceField> = struct_type
                        .fields
                        .iter()
                        .map(|f| {
                            let nested = match &f.ty {
                                MirType::Struct(inner) => Some(inner.name),
                                _ => None,
                            };
                            MirInstanceField {
                                name: f.name,
                                ty: f.ty.clone(),
                                offset: f.offset,
                                nested_instance: nested,
                                init: None,
                            }
                        })
                        .collect();

                    instance_types.push(MirInstanceType {
                        name: super::monomorphize::qualified_pou_ident(
                            db,
                            hir::hir_ty::ty::Type::Class(*class),
                        ),
                        fields: inst_fields,
                        size: struct_type.size,
                        align: struct_type.align,
                    });
                }

                // Lower methods
                let method_funcs = lower_class(
                    db,
                    *class,
                    next_fn_idx,
                    &mut memory_layout,
                    string_pool.clone(),
                )?;
                for mf in method_funcs {
                    function_indices.insert(mf.name, mf.index);
                    next_fn_idx += 1;
                    functions.push(mf);
                }
            }

            _ => {}
        }
    }

    // Phase 3: programs. A PROGRAM lowers like an FB; instances are allocated
    // per program configuration when the schedule is built.
    let mut program_infos: FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        crate::schedule::ProgramInfo<'db>,
    > = FxHashMap::default();
    for (program, _ns_prefix) in all_programs.iter() {
        let (mir_func, prog_type) = lower_program(
            db,
            **program,
            next_fn_idx,
            &mut memory_layout,
            string_pool.clone(),
        )?;
        let body_fn = mir_func.name;
        if let crate::types::MirType::Struct(struct_type) = &prog_type {
            program_infos.insert(
                program.name(db),
                crate::schedule::ProgramInfo {
                    body_fn,
                    struct_type: struct_type.clone(),
                    decl: **program,
                },
            );
        }
        function_indices.insert(body_fn, next_fn_idx);
        next_fn_idx += 1;
        functions.push(mir_func);
    }

    // Sort test entries by path for deterministic output
    test_entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Allocate storage for every config/resource VAR_GLOBAL. Bodies referenced
    // these as `Local(name)`; a post-pass below rewrites them to `Global`.
    let mut global_table = build_global_table(db, all_configs, &mut memory_layout)?;

    // Build the CONFIGURATION's schedule: allocate one instance per program
    // configuration (recording its RETAIN fields) and resolve task periods.
    let schedule =
        crate::schedule::lower_schedule(db, all_configs, &mut memory_layout, &program_infos);

    let mut module = MirModule {
        functions,
        extern_functions,
        string_literals: Vec::new(),
        instance_types,
        function_indices,
        type_indices,
        memory_layout,
        string_data: Vec::new(),
        test_manifest: crate::test_manifest::TestManifest {
            tests: test_entries,
        },
        retain_base: 0,
        retain_size: 0,
        schedule,
    };

    // Phase 4: Monomorphization - discovers call sites, generates concrete copies
    if !any_functions.is_empty() {
        monomorphize(
            db,
            &mut module,
            &any_functions,
            &mut MirMemoryLayout::new(),
            string_pool.clone(),
        )?;
    }

    // Phase 4.5: Relocate RETAIN variables into one contiguous, host-snapshottable
    // band. Done AFTER monomorphization so the band sits above every variable,
    // instance, and monomorphized-slot allocation; then patch the moved addresses
    // into each function's locals — the only place a variable's absolute address
    // is stored (body statements resolve addresses via the local map at codegen).
    let retain_band = module.memory_layout.finalize_retain_band();
    if retain_band.size > 0 {
        for func in &mut module.functions {
            for local in &mut func.locals {
                if let crate::function::MirStorage::Memory { address, .. } = &mut local.storage
                    && let Some(&new_addr) = retain_band.remap.get(address)
                {
                    *address = new_addr;
                }
            }
        }
        // A retain-holding program instance is relocated whole; patch its base
        // so the tasks pass the band address as `this`.
        if let Some(sched) = &mut module.schedule {
            for task in &mut sched.tasks {
                for inst in &mut task.programs {
                    if let Some(&new_addr) = retain_band.remap.get(&inst.instance_addr) {
                        inst.instance_addr = new_addr;
                    }
                }
            }
        }
        // RETAIN globals are relocated into the band too; patch their addresses.
        for (addr, _) in global_table.values_mut() {
            if let Some(&new_addr) = retain_band.remap.get(&*addr) {
                *addr = new_addr;
            }
        }
    }
    module.retain_base = retain_band.base;
    module.retain_size = retain_band.size;

    // Phase 4.6: synthesize one entry function per scheduled task (cooperative
    // model B — the runtime calls these). Each `__task_<i>` runs its task's
    // program instances in order, calling `Type$__body__(this = instance_addr)`.
    // Done AFTER the retain relocation so instance addresses are final, and
    // after monomorphization so function indices continue its contiguous scheme.
    if let Some(sched) = module.schedule.clone() {
        let mut idx = module.functions.len() as u32 + module.extern_functions.len() as u32;
        for (i, task) in sched.tasks.iter().enumerate() {
            let entry = hir::hir_def::interned::identifier::Ident::new(
                db,
                compact_str::CompactString::from(format!("__task_{i}")),
            );
            let body = task
                .programs
                .iter()
                .map(|inst| {
                    crate::stmt::MirStmt::Call(crate::expr::MirCall {
                        callee: inst.body_fn,
                        callee_index: 0, // resolved by name at codegen
                        args: vec![crate::expr::MirCallArg {
                            value: crate::expr::MirExpr::Constant(crate::expr::MirConstant::I32(
                                inst.instance_addr as i32,
                            )),
                            kind: crate::expr::MirArgKind::ByValue,
                        }],
                        return_type: crate::types::MirType::Void,
                        output_bindings: Vec::new(),
                    })
                })
                .collect();
            module.functions.push(crate::function::MirFunction {
                name: entry,
                origin_name: entry,
                index: idx,
                params: Vec::new(),
                return_type: None,
                locals: Vec::new(),
                body,
                linkage: crate::function::MirLinkage::Export,
                is_test: false,
                export_name: Some(compact_str::CompactString::from(format!("__task_{i}"))),
            });
            module.function_indices.insert(entry, idx);
            idx += 1;
        }
    }

    // Rewrite `Local(name)` -> `Global` for every global a body referenced,
    // now that addresses are final.
    for func in &mut module.functions {
        resolve_global_places(func, &global_table);
    }

    // Generate `__init`: write each scalar constant initializer (config/resource
    // VAR_GLOBALs + program instance fields) to its FINAL address exactly once.
    // The runtime calls `__init` after instantiation and BEFORE restoring retain,
    // so a RETAIN var's initializer is its cold-start value (overridden on warm
    // start). Addresses are final here (post-relocation). Functions keep using
    // the prepend pattern (stateless), so they're not included.
    let init_stmts = collect_const_inits(
        db,
        all_configs,
        &global_table,
        &module.schedule,
        &program_infos,
        &string_pool,
    )?;
    if !init_stmts.is_empty() {
        let idx = module.functions.len() as u32 + module.extern_functions.len() as u32;
        let name =
            hir::hir_def::interned::identifier::Ident::new(db, compact_str::CompactString::from("__init"));
        module.functions.push(crate::function::MirFunction {
            name,
            origin_name: name,
            index: idx,
            params: Vec::new(),
            return_type: None,
            locals: Vec::new(),
            body: init_stmts,
            linkage: crate::function::MirLinkage::Export,
            is_test: false,
            export_name: Some(compact_str::CompactString::from("__init")),
        });
        module.function_indices.insert(name, idx);
    }

    // End of all static memory, captured after phases 4/4.5 so the string
    // pool is placed past everything.
    let static_mem_end = module.memory_layout.total_size();

    // Phase 5: Rebase string pool to start AFTER all static memory allocations,
    // then extract interned string data into the module.
    {
        let mut pool = string_pool.borrow_mut();
        // Shift all string entry offsets by the static memory size
        for (offset, _) in pool.entries.iter_mut() {
            *offset += static_mem_end;
        }
    }
    module.string_data = std::mem::take(&mut string_pool.borrow_mut().entries)
        .into_iter()
        .collect();

    // Also rebase string literal offsets in all function bodies
    let string_base = static_mem_end;
    for func in &mut module.functions {
        rebase_string_offsets(&mut func.body, string_base);
    }

    Ok(module)
}

/// Build test cases from a function's `{case(...)}` pragmas.
fn build_test_cases<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    base_export: &str,
) -> Vec<crate::test_manifest::TestCase> {
    use crate::test_manifest::{TestCase, TestValue};
    use hir::hir_def::expressions::expression::{ExprKind, ParamAssignKind, PrimaryExpr};

    hir::hir_def::pous::pragma::cases(func.pragmas(db))
        .iter()
        .enumerate()
        .map(|(i, case_params)| {
            let args: Vec<TestValue> = case_params
                .iter()
                .filter_map(|param| {
                    let value_expr = match param.kind(db) {
                        ParamAssignKind::NonFormal { value } => value,
                        ParamAssignKind::FormalInput { value, .. } => value,
                        _ => return None,
                    };
                    match value_expr.expr(db) {
                        ExprKind::PrimaryExpr(PrimaryExpr::Literal(elem)) => {
                            Some(elementary_to_test_value(db, elem))
                        }
                        _ => None,
                    }
                })
                .collect();

            TestCase {
                export: format!("{}$case_{}", base_export, i),
                args,
            }
        })
        .collect()
}

/// Convert an HIR Elementary literal to a TestValue.
fn elementary_to_test_value(
    db: &dyn WorkspaceDataBase,
    elem: &hir::hir_def::expressions::expression::Elementary,
) -> crate::test_manifest::TestValue {
    use crate::test_manifest::TestValue;
    use hir::hir_def::expressions::expression::Elementary;

    match elem {
        Elementary::Bool(ident) => {
            let text = ident.text(db);
            TestValue::Bool(text.eq_ignore_ascii_case("TRUE") || text == "1")
        }
        Elementary::SInt(int)
        | Elementary::Int(int)
        | Elementary::DInt(int)
        | Elementary::USInt(int)
        | Elementary::UInt(int)
        | Elementary::UDInt(int)
        | Elementary::Byte(int)
        | Elementary::Word(int)
        | Elementary::DWord(int) => TestValue::I32(int.as_i32(db).unwrap_or(0)),
        Elementary::LInt(int) | Elementary::ULInt(int) | Elementary::LWord(int) => {
            TestValue::I64(int.as_i64(db).unwrap_or(0))
        }
        Elementary::Real(ident) | Elementary::InferFloat(ident) => {
            let text = ident.text(db);
            TestValue::F32(text.parse().unwrap_or(0.0))
        }
        Elementary::LReal(ident) => {
            let text = ident.text(db);
            TestValue::F64(text.parse().unwrap_or(0.0))
        }
        Elementary::InferInteger(int) => TestValue::I32(int.as_i32(db).unwrap_or(0)),
        _ => TestValue::I32(0), // fallback for time/date/string
    }
}

/// Recursively collect all POUs from a namespace and its children.
/// Each item is paired with its dot-separated namespace path prefix.
fn collect_namespace_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    ns: &hir::hir_def::namespace::NamespaceDecl<'db>,
    pous: &mut Vec<(&'db Pou<'db>, Option<String>)>,
) {
    let ns_prefix = ns.path(db).to_string(db);
    for pou in ns.pous(db).iter() {
        pous.push((pou, Some(ns_prefix.clone())));
    }
    for child_ns in ns.namespaces(db).iter() {
        collect_namespace_pous(db, child_ns, pous);
    }
}

/// Lower a non-ANY extern function to MirExternFunction.
fn lower_extern_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    extern_decl: &hir::hir_def::extern_decl::ExternDecl<'db>,
    index: u32,
) -> Result<MirExternFunction, LowerTypeError> {
    let mut params = Vec::new();
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input => {
                let ty = lower_type(db, var.spec(db).infer(db))?;
                params.push(MirParam {
                    name: var.name(db),
                    ty,
                    kind: MirParamKind::Input,
                });
            }
            VariableKind::InOut => {
                let ty = lower_type(db, var.spec(db).infer(db))?;
                params.push(MirParam {
                    name: var.name(db),
                    ty: MirType::Pointer(Box::new(ty)),
                    kind: MirParamKind::InOut,
                });
            }
            _ => {}
        }
    }

    let return_type = func
        .return_type(db)
        .map(|spec| lower_type(db, spec.infer(db)))
        .transpose()?;

    Ok(MirExternFunction {
        name: super::monomorphize::qualified_pou_ident(db, hir::hir_ty::ty::Type::Function(func)),
        index,
        module: extern_decl.module.clone(),
        import_name: extern_decl.name.clone(),
        params,
        return_type,
        monomorphized_from: None,
    })
}

/// Lower a {wasm} intrinsic function to a MirFunction.
/// The body is a single assignment: result := cast(param).
pub fn lower_wasm_intrinsic<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    wasm_decl: &hir::hir_def::extern_decl::WasmDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
) -> Result<crate::function::MirFunction, LowerTypeError> {
    use crate::expr::*;
    use crate::function::*;
    use crate::stmt::*;
    use hir::hir_def::pous::variable::VariableKind;

    // Parse the instruction to determine from/to types
    let _instruction = wasm_decl.instruction.as_str();

    // Build params. Handles all three modes:
    //   - Input   → param value
    //   - InOut   → caller's buffer, mutable
    //   - Output  → caller's buffer, write-only
    // Reference modes are wrapped in `MirType::Pointer` so the codegen
    // knows to flatten STRING references into the (addr, cap) pair.
    let mut params = Vec::new();
    let mut param_name = None;
    let mut param_elem = None;
    let mut input_count: u32 = 0;
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input => {
                let ty = lower_type(db, var.spec(db).infer(db))?;
                if let MirType::Elementary(e) = &ty {
                    param_elem = Some(*e);
                }
                param_name = Some(var.name(db));
                input_count += 1;
                params.push(MirParam {
                    name: var.name(db),
                    ty,
                    kind: MirParamKind::Input,
                });
            }
            VariableKind::InOut | VariableKind::Output => {
                let ty = lower_type(db, var.spec(db).infer(db))?;
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

    let return_type = func
        .return_type(db)
        .map(|spec| lower_type(db, spec.infer(db)))
        .transpose()?;

    let mut return_elem = None;
    if let Some(MirType::Elementary(e)) = &return_type {
        return_elem = Some(*e);
    }

    // Return local - needed so the local map has an entry for the return
    // variable. STRING returns require Memory storage so the codegen can
    // allocate the embedded buffer and producer builtins can write
    // directly into it; everything else uses a WASM-local scalar.
    //
    // The wasm-local index for a scalar return slot is the total number
    // of wasm slots the params consume, **not** `params.len()`: a STRING
    // input flattens to two slots. Using `params.len()` would alias the
    // return slot with the second slot of a STRING param.
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = params
        .iter()
        .map(|p| crate::lower::lower_func::param_wasm_width(&p.ty, p.kind))
        .sum();
    if let Some(ref ret_ty) = return_type {
        let storage = crate::lower::lower_func::allocate_local_storage(
            func.name(db),
            ret_ty,
            /* is_address_taken = */ false,
            &mut next_local_idx,
            memory_layout,
        );
        locals.push(crate::function::MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: crate::function::MirLocalKind::Var,
            storage,
            // Synthetic return slot for a stateless intrinsic/extern shim.
            var_storage: crate::function::MirVariableStorage::Automatic,
        });
    }

    // Build body. Two shapes:
    //
    // 1. **Single elementary input → elementary output**: emit a Cast so the
    //    cast emitter (mir_cast.rs) can translate to the appropriate WASM
    //    op or unit-conversion sequence. Covers all of Convert.st today.
    //
    // 2. **Anything else** (multiple inputs, STRING params, etc.): emit a
    //    `WasmIntrinsic` whose instruction string is consumed by the
    //    codegen - either dispatched into `BUILTIN_NAMES` (graft a call to
    //    a `wasm_builtins` function) or matched in `emit_wasm_instruction`.
    //    Used by the new string-inspection helpers (`str_byte_len`,
    //    `str_char_count`, etc.) where the cast machinery doesn't apply.
    //
    // The single-input gate (`input_count == 1`) matters: the loop above
    // overwrites `param_name`/`param_elem` on each iteration, so without
    // it a multi-input pragma like `{wasm 'rk.div_i32_checked'
    // (params a b) (result r)}` would silently fall into the Cast branch
    // and emit `r := b`, dropping `a` and the builtin call entirely.
    let body = if input_count == 1
        && let (Some(p_name), Some(from), Some(to)) = (param_name, param_elem, return_elem)
    {
        let load = MirExpr::Load(MirPlace::Local(p_name), MirType::Elementary(from));
        let cast_expr = if from == to {
            load
        } else {
            MirExpr::Cast {
                expr: Box::new(load),
                from,
                to,
            }
        };
        vec![MirStmt::Assign {
            target: MirPlace::Local(func.name(db)),
            value: cast_expr,
        }]
    } else {
        let param_names: Vec<_> = params.iter().map(|p| p.name).collect();
        vec![MirStmt::WasmIntrinsic {
            instruction: wasm_decl.instruction.clone(),
            params: param_names,
            result: if return_type.is_some() {
                Some(func.name(db))
            } else {
                None
            },
        }]
    };

    Ok(MirFunction {
        name: super::monomorphize::qualified_pou_ident(db, hir::hir_ty::ty::Type::Function(func)),
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

/// Find extern pragma in a function's statements.
fn find_extern_decl<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Option<hir::hir_def::extern_decl::ExternDecl<'db>> {
    func.statements(db).iter().find_map(|stmt| {
        if let StmtKind::ExternPragma(decl) = stmt.stmt(db) {
            Some(decl.clone())
        } else {
            None
        }
    })
}

/// Rebase all StringLiteral offsets in MIR statements by adding `base` to each offset.
fn rebase_string_offsets(stmts: &mut [crate::stmt::MirStmt], base: u32) {
    use crate::stmt::MirStmt;

    for stmt in stmts {
        match stmt {
            MirStmt::Assign { value, .. } => rebase_expr(value, base),
            MirStmt::Call(call) => {
                for arg in &mut call.args {
                    rebase_expr(&mut arg.value, base);
                }
            }
            MirStmt::FbCall { input_writes, .. } => {
                for (_, value, _) in input_writes {
                    rebase_expr(value, base);
                }
            }
            MirStmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                rebase_expr(condition, base);
                rebase_string_offsets(then_body, base);
                if let Some(else_body) = else_body {
                    rebase_string_offsets(else_body, base);
                }
            }
            MirStmt::While {
                condition, body, ..
            } => {
                rebase_expr(condition, base);
                rebase_string_offsets(body, base);
            }
            MirStmt::For { body, .. } => {
                rebase_string_offsets(body, base);
            }
            MirStmt::Repeat {
                condition, body, ..
            } => {
                rebase_expr(condition, base);
                rebase_string_offsets(body, base);
            }
            MirStmt::Case {
                arms, else_body, ..
            } => {
                for arm in arms {
                    rebase_string_offsets(&mut arm.body, base);
                }
                if let Some(else_body) = else_body {
                    rebase_string_offsets(else_body, base);
                }
            }
            MirStmt::Raise { message } => rebase_expr(message, base),
            _ => {}
        }
    }
}

fn rebase_expr(expr: &mut crate::expr::MirExpr, base: u32) {
    use crate::expr::MirExpr;
    match expr {
        MirExpr::StringLiteral { offset, .. } => {
            *offset += base;
        }
        MirExpr::BinOp { lhs, rhs, .. } => {
            rebase_expr(lhs, base);
            rebase_expr(rhs, base);
        }
        MirExpr::UnaryOp { expr: operand, .. } => {
            rebase_expr(operand, base);
        }
        MirExpr::Call(call) => {
            for arg in &mut call.args {
                rebase_expr(&mut arg.value, base);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// VAR_GLOBAL lowering
// ---------------------------------------------------------------------------

type GlobalTable<'db> =
    FxHashMap<hir::hir_def::interned::identifier::Ident, (u32, crate::types::MirType)>;

/// Allocate a linear-memory slot for every config/resource VAR_GLOBAL and build
/// the symbol table (name -> (address, type)). RETAIN globals are recorded into
/// the host-snapshottable band. Located (`AT %…`) globals are treated as plain
/// storage for now — hardware mapping is not implemented.
fn build_global_table<'db>(
    db: &'db dyn WorkspaceDataBase,
    configs: &[hir::hir_def::config::ConfigDecl<'db>],
    memory_layout: &mut MirMemoryLayout,
) -> Result<GlobalTable<'db>, LowerTypeError> {
    use hir::hir_def::config::ConfigResource;
    let mut table = GlobalTable::default();
    for config in configs {
        for v in config.variables(db) {
            add_global(db, v, memory_layout, &mut table)?;
        }
        for res in config.resources(db) {
            if let ConfigResource::Resource(r) = res {
                for v in r.variables(db) {
                    add_global(db, v, memory_layout, &mut table)?;
                }
            }
        }
    }
    Ok(table)
}

fn add_global<'db>(
    db: &'db dyn WorkspaceDataBase,
    v: &hir::hir_def::pous::variable::VariableDecl<'db>,
    memory_layout: &mut MirMemoryLayout,
    table: &mut GlobalTable<'db>,
) -> Result<(), LowerTypeError> {
    use hir::hir_ty::infer::Infer;
    let ty = super::lower_type::lower_type(db, v.spec(db).infer(db))?;
    let size = ty.size_bytes();
    let align = ty.alignment();
    let addr = memory_layout.allocate(v.name(db), size, align, crate::memory::MirAllocKind::Variable);
    if v.qualifier(db).contains(hir::Qualifier::RETAIN) {
        memory_layout.record_retain(v.name(db), addr, size, align);
    }
    table.insert(v.name(db), (addr, ty));
    Ok(())
}

/// Rewrite `MirPlace::Local(name)` references to globals into `MirPlace::Global`.
/// A body lowers a global reference (direct or VAR_EXTERNAL) as `Local(name)`,
/// since the address isn't known at body-lowering time. A name is a global iff
/// it's in `globals` AND not an actual local/param of this function (locals
/// shadow globals).
fn resolve_global_places(func: &mut crate::function::MirFunction, globals: &GlobalTable<'_>) {
    use rustc_hash::FxHashSet;
    let mut locals: FxHashSet<_> = func.locals.iter().map(|l| l.name).collect();
    locals.extend(func.params.iter().map(|p| p.name));
    for stmt in &mut func.body {
        rewrite_globals_stmt(stmt, globals, &locals);
    }
}

fn rewrite_globals_place(
    place: &mut crate::expr::MirPlace,
    globals: &GlobalTable<'_>,
    locals: &rustc_hash::FxHashSet<hir::hir_def::interned::identifier::Ident>,
) {
    use crate::expr::MirPlace;
    match place {
        MirPlace::Local(name) => {
            if !locals.contains(name)
                && let Some((address, ty)) = globals.get(name)
            {
                *place = MirPlace::Global {
                    address: *address,
                    ty: ty.clone(),
                };
            }
        }
        MirPlace::Field { base, .. } | MirPlace::Deref { base, .. } => {
            rewrite_globals_place(base, globals, locals);
        }
        MirPlace::Index { base, index, .. } => {
            rewrite_globals_place(base, globals, locals);
            rewrite_globals_expr(index, globals, locals);
        }
        MirPlace::ThisField { .. } | MirPlace::Global { .. } => {}
    }
}

fn rewrite_globals_expr(
    expr: &mut crate::expr::MirExpr,
    globals: &GlobalTable<'_>,
    locals: &rustc_hash::FxHashSet<hir::hir_def::interned::identifier::Ident>,
) {
    use crate::expr::MirExpr;
    match expr {
        MirExpr::Load(place, _) | MirExpr::AddrOf(place) => {
            rewrite_globals_place(place, globals, locals);
        }
        MirExpr::BinOp { lhs, rhs, .. } => {
            rewrite_globals_expr(lhs, globals, locals);
            rewrite_globals_expr(rhs, globals, locals);
        }
        MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
            rewrite_globals_expr(expr, globals, locals);
        }
        MirExpr::Call(call) => {
            for arg in &mut call.args {
                rewrite_globals_expr(&mut arg.value, globals, locals);
            }
        }
        MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => {}
    }
}

fn rewrite_globals_stmt(
    stmt: &mut crate::stmt::MirStmt,
    globals: &GlobalTable<'_>,
    locals: &rustc_hash::FxHashSet<hir::hir_def::interned::identifier::Ident>,
) {
    use crate::stmt::MirStmt;
    match stmt {
        MirStmt::Assign { target, value } => {
            rewrite_globals_place(target, globals, locals);
            rewrite_globals_expr(value, globals, locals);
        }
        MirStmt::Call(call) => {
            for arg in &mut call.args {
                rewrite_globals_expr(&mut arg.value, globals, locals);
            }
            for binding in &mut call.output_bindings {
                rewrite_globals_place(&mut binding.target, globals, locals);
            }
        }
        MirStmt::FbCall {
            instance,
            input_writes,
            output_reads,
            ..
        } => {
            rewrite_globals_place(instance, globals, locals);
            for (_, value, _) in input_writes {
                rewrite_globals_expr(value, globals, locals);
            }
            for (_, target, _) in output_reads {
                rewrite_globals_place(target, globals, locals);
            }
        }
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            rewrite_globals_expr(condition, globals, locals);
            rewrite_globals_body(then_body, globals, locals);
            for (cond, body) in else_ifs {
                rewrite_globals_expr(cond, globals, locals);
                rewrite_globals_body(body, globals, locals);
            }
            if let Some(body) = else_body {
                rewrite_globals_body(body, globals, locals);
            }
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            rewrite_globals_expr(selector, globals, locals);
            for arm in arms {
                rewrite_globals_body(&mut arm.body, globals, locals);
            }
            if let Some(body) = else_body {
                rewrite_globals_body(body, globals, locals);
            }
        }
        MirStmt::For {
            start, end, step, body, ..
        } => {
            rewrite_globals_expr(start, globals, locals);
            rewrite_globals_expr(end, globals, locals);
            rewrite_globals_expr(step, globals, locals);
            rewrite_globals_body(body, globals, locals);
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            rewrite_globals_expr(condition, globals, locals);
            rewrite_globals_body(body, globals, locals);
        }
        MirStmt::Raise { message } => rewrite_globals_expr(message, globals, locals),
        MirStmt::Return
        | MirStmt::Exit
        | MirStmt::Continue
        | MirStmt::MemStore { .. }
        | MirStmt::WasmIntrinsic { .. }
        | MirStmt::DebugTrap { .. } => {}
    }
}

fn rewrite_globals_body(
    body: &mut [crate::stmt::MirStmt],
    globals: &GlobalTable<'_>,
    locals: &rustc_hash::FxHashSet<hir::hir_def::interned::identifier::Ident>,
) {
    for stmt in body {
        rewrite_globals_stmt(stmt, globals, locals);
    }
}

/// Collect `Assign { Global{addr}, <const> }` statements for every scalar
/// constant initializer that must run once at startup: config/resource
/// VAR_GLOBALs and program-instance fields. Addresses are taken from the
/// already-finalized global table and instance bases.
fn collect_const_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    configs: &[hir::hir_def::config::ConfigDecl<'db>],
    global_table: &GlobalTable<'db>,
    schedule: &Option<crate::schedule::MirSchedule>,
    program_infos: &FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        crate::schedule::ProgramInfo<'db>,
    >,
    string_pool: &std::rc::Rc<std::cell::RefCell<crate::lower::lower_expr::StringPool>>,
) -> Result<Vec<crate::stmt::MirStmt>, LowerTypeError> {
    use hir::hir_def::config::ConfigResource;

    let mut stmts = Vec::new();

    // Config/resource VAR_GLOBALs.
    for config in configs {
        let mut globals: Vec<&hir::hir_def::pous::variable::VariableDecl<'db>> =
            config.variables(db).iter().collect();
        for res in config.resources(db) {
            if let ConfigResource::Resource(r) = res {
                globals.extend(r.variables(db).iter());
            }
        }
        for v in globals {
            if let Some((addr, ty)) = global_table.get(&v.name(db))
                && let Some(init) = v.init(db)
            {
                // All-or-nothing: only commit if the whole initializer lowers.
                let mut local = Vec::new();
                if super::lower_func::lower_init_into(db, *addr, ty, init, &mut local, string_pool)?
                {
                    stmts.extend(local);
                }
            }
        }
    }

    // Program instance fields.
    if let Some(sched) = schedule {
        for task in &sched.tasks {
            for inst in &task.programs {
                let Some(info) = program_infos.get(&inst.prog_name) else {
                    continue;
                };
                for field in &info.struct_type.fields {
                    let Some(var) = info
                        .decl
                        .variables(db)
                        .iter()
                        .find(|v| v.name(db) == field.name)
                    else {
                        continue;
                    };
                    let Some(init) = var.init(db) else { continue };
                    let mut local = Vec::new();
                    if super::lower_func::lower_init_into(
                        db,
                        inst.instance_addr + field.offset,
                        &field.ty,
                        init,
                        &mut local,
                        string_pool,
                    )? {
                        stmts.extend(local);
                    }
                }
            }
        }
    }

    Ok(stmts)
}
