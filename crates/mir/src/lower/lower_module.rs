use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::{
    pous::{function::Function, pou::Pou, variable::VariableKind},
    semantic_index::SemanticIndex,
};
use hir::hir_ty::infer::Infer;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    MirInstanceField, MirInstanceType, MirModule,
    function::{MirExternFunction, MirParam, MirParamKind},
    lower::{
        lower_func::{lower_class, lower_function, lower_function_block, lower_program},
        lower_type::{LowerTypeError, lower_type},
    },
    memory::MirMemoryLayout,
    types::MirType,
};

/// Workspace-relative file and 1-based line of a test FUNCTION, for the
/// manifest; relative so the artifact is reproducible across machines.
fn test_location<'db>(db: &'db dyn WorkspaceDataBase, func: Function<'db>) -> (String, u32) {
    use db::workspace::Workspace;
    use hir::{HasName, HirNodeInfo};

    let file = func.get_scope_id(db).file(db);
    let line = func.get_name_span(db).start_point.row as u32 + 1;

    let Ok(path) = file.url(db).to_file_path() else {
        return (String::new(), line);
    };
    let workspace = Workspace::try_get(db);
    if let Some(root) = workspace.and_then(|w| w.workspace_folder(db))
        && let Ok(rel) = path.strip_prefix(root)
    {
        return (rel.to_string_lossy().into_owned(), line);
    }
    if let Some(lib) = workspace.and_then(|w| w.library_path(db))
        && let Ok(rel) = path.strip_prefix(lib)
    {
        return (format!("<lib>/{}", rel.to_string_lossy()), line);
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (name, line)
}

/// Whether `node` was declared by the workspace rather than the library,
/// by the rule the LSP uses to scope its requests.
fn is_workspace<'db>(db: &'db dyn WorkspaceDataBase, node: impl hir::HirNodeInfo<'db>) -> bool {
    let file = node.get_scope_id(db).file(db);
    db.get_files().contains_key(file.url(db))
}

/// Only the workspace's tests are compiled: `rk test` runs those, and a
/// library's are its own business.
fn is_workspace_test<'db>(db: &'db dyn WorkspaceDataBase, func: Function<'db>) -> bool {
    is_workspace(db, func)
}

/// Pin a codegen error to a POU declaration; the innermost location
/// already attached wins.
fn at_pou<'db, T>(
    db: &'db dyn WorkspaceDataBase,
    node: impl hir::HirNodeInfo<'db>,
    result: Result<T, LowerTypeError>,
) -> Result<T, LowerTypeError> {
    result.map_err(|e| e.with_location(node.get_scope_id(db).file(db), node.get_span(db)))
}

/// Lower multiple HIR semantic indices (from multiple files) into a single MirModule.
pub fn lower_modules<'db>(
    db: &'db dyn WorkspaceDataBase,
    indices: &[&'db SemanticIndex<'db>],
) -> Result<MirModule, LowerTypeError> {
    // Every POU and program, each with its optional namespace prefix.
    let mut all_pous: Vec<(&Pou<'db>, Option<String>)> = Vec::new();
    let mut all_programs: Vec<(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)> =
        Vec::new();
    // A workspace declares one CONFIGURATION (E1402), so every block is a
    // fragment of it, lowered together in file order.
    let mut fragments: Vec<hir::hir_def::config::ConfigDecl<'db>> = Vec::new();
    for index in indices {
        all_pous.extend(index.global_pous.iter().map(|p| (p, None)));
        all_programs.extend(index.programs.iter().map(|p| (p, None)));
        // The workspace's fragments only: a library's configuration is its
        // author's PLC.
        fragments.extend(
            index
                .configs
                .iter()
                .filter(|c| is_workspace(db, **c))
                .copied(),
        );
        // The index's namespace list is flat, so each namespace's POUs are
        // collected exactly once.
        for ns in index.namespaces.iter() {
            let ns_prefix = ns.path(db).to_string(db);
            for pou in ns.pous(db).iter() {
                all_pous.push((pou, Some(ns_prefix.clone())));
            }
        }
    }
    lower_module_from_pous(db, &all_pous, &all_programs, &fragments)
}

/// Lower a complete HIR semantic index into a MirModule.
pub fn lower_module<'db>(
    db: &'db dyn WorkspaceDataBase,
    index: &SemanticIndex<'db>,
) -> Result<MirModule, LowerTypeError> {
    // Namespaced POUs included, as the multi-index path collects them.
    let mut all_pous: Vec<(&Pou<'db>, Option<String>)> =
        index.global_pous.iter().map(|p| (p, None)).collect();
    for ns in index.namespaces.iter() {
        let ns_prefix = ns.path(db).to_string(db);
        for pou in ns.pous(db).iter() {
            all_pous.push((pou, Some(ns_prefix.clone())));
        }
    }
    lower_module_from_pous(
        db,
        &all_pous,
        &index.programs.iter().map(|p| (p, None)).collect::<Vec<_>>(),
        &index.configs.iter().copied().collect::<Vec<_>>(),
    )
}

fn lower_module_from_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
    // Every block of the workspace's one CONFIGURATION, in file order.
    config: &[hir::hir_def::config::ConfigDecl<'db>],
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

    // Phase B: interface-parameter monomorphization. Function specializations
    // emit at module level, method specializations with their declaring
    // owner's method set.
    let (iface_instances, iface_call_rewrites) =
        super::mono_iface::collect_iface_instantiations(db, all_pous, all_programs);

    // Phase C: one specialization per (variadic function, argument count)
    // called; call sites re-read the resolution in `variadic_arity_of`.
    let (arity_instances, _arity_call_rewrites) =
        super::mono_arity::collect_arity_instantiations(db, all_pous, all_programs);
    let mut arity_by_func: FxHashMap<
        hir::hir_def::pous::function::Function<'db>,
        Vec<&super::mono_arity::ArityInstance<'db>>,
    > = FxHashMap::default();
    for inst in &arity_instances {
        arity_by_func.entry(inst.func).or_default().push(inst);
    }
    let mut iface_by_func: FxHashMap<
        hir::hir_def::pous::function::Function<'db>,
        Vec<&super::mono_iface::IfaceInstance<'db>>,
    > = FxHashMap::default();
    let mut iface_methods_by_owner: FxHashMap<
        Pou<'db>,
        Vec<&super::mono_iface::IfaceInstance<'db>>,
    > = FxHashMap::default();
    for inst in &iface_instances {
        match inst.target {
            super::mono_iface::IfaceTarget::Function(f) => {
                iface_by_func.entry(f).or_default().push(inst);
            }
            super::mono_iface::IfaceTarget::Method { owner, .. } => {
                iface_methods_by_owner.entry(owner).or_default().push(inst);
            }
        }
    }

    // Collect test entries for the manifest
    let mut test_entries: Vec<crate::test_manifest::TestEntry> = Vec::new();

    // Phase 1: Process imports first (extern functions get lower indices)
    for (pou, _ns_prefix) in all_pous.iter() {
        if let Pou::Function(func) = pou {
            use hir::HasPragmas;
            let Some((_, extern_decl)) = func.extern_pragma(db) else {
                continue;
            };

            let mir_ext = lower_extern_function(db, *func, extern_decl, next_fn_idx)?;
            function_indices.insert(mir_ext.name, next_fn_idx);
            next_fn_idx += 1;
            extern_functions.push(mir_ext);
        }
    }

    // Phase 2: Process local functions
    for (pou, _ns_prefix) in all_pous.iter() {
        match pou {
            Pou::Function(func) => {
                let func_id = super::naming::mir_function_symbol(db, *func);
                // Skip already-processed externs
                if function_indices.contains_key(&func_id) {
                    continue;
                }

                // Externs were handled in phase 1.
                {
                    use hir::HasPragmas;
                    if func.extern_pragma(db).is_some() {
                        continue;
                    }
                }

                // A library's tests are left out. Nothing calls a test but
                // the runner (E1007 refuses the reference), so nothing
                // misses them.
                let is_test = hir::hir_def::pous::pragma::is_test(db, func.pragmas(db));
                if is_test && !is_workspace_test(db, *func) {
                    continue;
                }

                // Phase B: a function with an interface param has no generic form; one
                // copy per concrete instantiation (`drive$Worker`).
                let has_iface_param = func
                    .variables(db)
                    .iter()
                    .any(|v| super::mono_iface::is_interface_param(db, v));
                if has_iface_param {
                    for inst in iface_by_func.get(func).into_iter().flatten() {
                        let mut mir_func = lower_function(
                            db,
                            *func,
                            next_fn_idx,
                            &mut memory_layout,
                            string_pool.clone(),
                            Some(&inst.iface_subs),
                            // A specialization's body uses its own rewrites.
                            &inst.call_rewrites,
                            None,
                        )?;
                        mir_func.name = inst.mangled_name;
                        function_indices.insert(mir_func.name, next_fn_idx);
                        next_fn_idx += 1;
                        functions.push(mir_func);
                    }
                    continue;
                }

                // Phase C: a variadic function is emitted once per arity called
                // (`sum_all$3`); an uncalled one emits nothing.
                if super::mono_arity::variadic_param(db, *func).is_some() {
                    for inst in arity_by_func.get(func).into_iter().flatten() {
                        let mut mir_func = lower_function(
                            db,
                            *func,
                            next_fn_idx,
                            &mut memory_layout,
                            string_pool.clone(),
                            None,
                            &iface_call_rewrites,
                            Some(inst.arity),
                        )?;
                        mir_func.name = inst.mangled_name;
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
                    None,
                    &iface_call_rewrites,
                    None,
                )?;
                // Export under the qualified MIR symbol, so overloads do not collide.
                mir_func.export_name = Some(mir_func.name.text(db).to_string().into());
                function_indices.insert(mir_func.name, next_fn_idx);
                next_fn_idx += 1;

                // Collect test entry if marked with {test}
                if is_test {
                    let export_name = mir_func
                        .export_name
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| mir_func.name.text(db).to_string());
                    let (file, line) = test_location(db, *func);
                    test_entries.push(crate::test_manifest::TestEntry {
                        path: export_name.clone(),
                        export: export_name,
                        file,
                        line,
                    });
                }

                functions.push(mir_func);
            }

            Pou::FunctionBlock(fb) => {
                // Emit one set of (instance type + body + methods) per FB, keyed
                // by its namespace-qualified name.
                let fb_qualified = super::naming::qualified_pou_ident(
                    db,
                    hir::hir_ty::ty::Type::FunctionBlock(*fb),
                );

                // Instance type.
                let fb_mir_type = at_pou(db, *fb, super::lower_type::lower_fb_type(db, *fb))?;
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
                        name: fb_qualified,
                        fields: inst_fields,
                        size: struct_type.size,
                        align: struct_type.align,
                    });
                }

                // Lower methods + __body__.
                let method_funcs = lower_function_block(
                    db,
                    *fb,
                    next_fn_idx,
                    &mut memory_layout,
                    string_pool.clone(),
                    &iface_call_rewrites,
                    iface_methods_by_owner
                        .get(&Pou::FunctionBlock(*fb))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                )?;
                for mf in method_funcs {
                    function_indices.insert(mf.name, mf.index);
                    next_fn_idx += 1;
                    functions.push(mf);
                }
            }

            Pou::Class(class) => {
                // Build instance type
                let class_type = at_pou(
                    db,
                    *class,
                    lower_type(db, hir::hir_ty::ty::Type::Class(*class)),
                )?;
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
                        name: super::naming::qualified_pou_ident(
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
                    &iface_call_rewrites,
                    iface_methods_by_owner
                        .get(&Pou::Class(*class))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
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
            &iface_call_rewrites,
        )?;
        let body_fn = mir_func.name;
        let crate::types::MirType::Struct(struct_type) = &prog_type else {
            // A program that lowered but never registered would be absent from
            // every schedule.
            return Err(LowerTypeError::UnsupportedType(format!(
                "PROGRAM '{}' lowered to a non-struct instance type",
                program.name(db).text(db)
            )));
        };
        // E0102 refuses duplicate PROGRAMs at check; lowering refuses them too.
        if program_infos
            .insert(
                program.name(db),
                crate::schedule::ProgramInfo {
                    body_fn,
                    struct_type: struct_type.clone(),
                    decl: **program,
                },
            )
            .is_some()
        {
            return Err(LowerTypeError::UnsupportedType(format!(
                "two PROGRAMs named '{}' reached lowering",
                program.name(db).text(db)
            )));
        }
        function_indices.insert(body_fn, next_fn_idx);
        next_fn_idx += 1;
        functions.push(mir_func);
    }

    // A program instance with connections runs through a function of its
    // own, which feeds its inputs and copies its outputs out around the body.
    for c in config {
        let schedule = &hir::hir_ty::config::infer_config_result(db, *c).schedule;
        for p in schedule
            .resources
            .iter()
            .flat_map(|r| r.tasks.iter())
            .flat_map(|t| t.programs.iter())
            .filter(|p| !p.connections.inputs.is_empty() || !p.connections.outputs.is_empty())
        {
            // `lower_schedule` refuses an instance with no lowered program.
            let Some(info) = program_infos.get(&p.program.name(db)) else {
                continue;
            };
            let func =
                super::connections::lower_connections(db, p, info, next_fn_idx, &string_pool)?;
            function_indices.insert(func.name, next_fn_idx);
            next_fn_idx += 1;
            functions.push(func);
        }
    }

    // Sort test entries by path for deterministic output
    test_entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Allocate storage for every configuration VAR_GLOBAL. Bodies referenced
    // these as `Local(name)`; a post-pass below rewrites them to `Global`.
    let mut global_table = build_global_table(db, config, all_programs, &mut memory_layout)?;

    // Bare addresses the bodies named get their cells before the bands are
    // carved, so they are laid out with the declared ones, and so do the ones
    // only VAR_CONFIG names: what a located instance variable points at.
    let config_locations: Vec<&hir::hir_ty::config::ConfigLocation<'db>> = config
        .iter()
        .flat_map(|c| {
            hir::hir_ty::config::infer_config_result(db, *c)
                .locations
                .iter()
        })
        .collect();
    let config_cells: Vec<hir::hir_def::pous::variable::LocatedAddress> = config_locations
        .iter()
        .map(|loc| config_cell(db, &loc.address).0)
        .collect();
    synthesize_bare_addresses(
        db,
        &mut functions,
        &config_cells,
        &mut memory_layout,
        &mut global_table,
    )?;

    // Build the CONFIGURATION's schedule: allocate one instance per program
    // configuration (recording its RETAIN fields) and resolve task periods.
    let schedule = crate::schedule::lower_schedule(db, config, &mut memory_layout, &program_infos)?;
    // The host calls the body of a function block a task runs.
    if let Some(sched) = &schedule {
        let run: FxHashSet<_> = sched
            .tasks
            .iter()
            .flat_map(|t| t.function_blocks.iter().map(|f| f.body_fn))
            .collect();
        for func in functions.iter_mut().filter(|f| run.contains(&f.name)) {
            func.linkage = crate::function::MirLinkage::Export;
        }
    }

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
        globals_base: 0,
        globals_size: 0,
        input_base: 0,
        input_size: 0,
        output_base: 0,
        output_size: 0,
        marker_base: 0,
        marker_size: 0,
        schedule,
        schedule_manifest: None,
        debug_symbols: crate::debug_symbols::DebugSymbols::new(),
        retain_map: debug_format::RetainMap::new(Vec::new()),
        located_map: debug_format::LocatedMap::new(Vec::new()),
        source_files: Vec::new(),
    };

    // Phase 4.5: relocate RETAIN variables into one contiguous band above
    // every other allocation, then patch the moved addresses into each
    // function's locals.
    let bands = module.memory_layout.finalize_bands();
    if !bands.remap.is_empty() {
        for func in &mut module.functions {
            for local in &mut func.locals {
                if let crate::function::MirStorage::Memory { address, .. } = &mut local.storage
                    && let Some(&new_addr) = bands.remap.get(address)
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
                    if let Some(&new_addr) = bands.remap.get(&inst.instance_addr) {
                        inst.instance_addr = new_addr;
                    }
                }
                for fb in &mut task.function_blocks {
                    if let Some(&new_addr) = bands.remap.get(&fb.program_addr) {
                        fb.program_addr = new_addr;
                    }
                }
            }
        }
        // Every global is relocated into the globals band; patch its address so
        // body references (resolved via the global table) hit the band.
        for (addr, _) in global_table.values_mut() {
            if let Some(&new_addr) = bands.remap.get(&*addr) {
                *addr = new_addr;
            }
        }
    }
    module.retain_base = bands.retain_base;
    module.retain_size = bands.retain_size;
    module.globals_base = bands.globals_base;
    module.globals_size = bands.globals_size;
    module.input_base = bands.input_base;
    module.input_size = bands.input_size;
    module.output_base = bands.output_base;
    module.output_size = bands.output_size;
    module.marker_base = bands.marker_base;
    module.marker_size = bands.marker_size;
    module.located_map = build_located_map(db, &bands.located, &global_table, &config_locations);

    // The debug-symbol table, now that every address is final; sorted by
    // path.
    let mut symbols = Vec::new();
    let mut array_syms = Vec::new();
    let mut containers = Vec::new();
    let mut type_table = crate::debug_symbols::TypeTable::new();
    if let Some(sched) = &module.schedule {
        for task in &sched.tasks {
            for inst in &task.programs {
                // `lower_schedule` refused any instance without a lowered program.
                let Some(info) = program_infos.get(&inst.prog_name) else {
                    return Err(LowerTypeError::UnsupportedType(format!(
                        "scheduled instance '{}' names program '{}' with no lowered info",
                        inst.inst_name.text(db),
                        inst.prog_name.text(db)
                    )));
                };
                // The instance itself: whose state a stopped frame is running on.
                containers.push(debug_format::ContainerSym {
                    path: inst.inst_name.text(db).to_string(),
                    address: inst.instance_addr,
                    global: false,
                    type_name: inst.prog_name.text(db).to_string(),
                });
                for f in &info.struct_type.fields {
                    let path = crate::debug_symbols::join_path(db, inst.inst_name.text(db), f.name);
                    // Per-field leaf budget, matching `collect_root`'s per-root budget.
                    crate::debug_symbols::collect_root(
                        db,
                        &path,
                        inst.instance_addr + f.offset,
                        &f.ty,
                        false, // program-instance field
                        &mut symbols,
                        &mut array_syms,
                        &mut type_table,
                    );
                    crate::debug_symbols::collect_containers(
                        db,
                        &path,
                        inst.instance_addr + f.offset,
                        &f.ty,
                        false,
                        &mut containers,
                    );
                }
                // A PROGRAM's located VAR is one cell every instance shares,
                // listed under the program's name (`P.count`). A debugger
                // browsing an instance finds it under the instance's path
                // too, at that same cell.
                for var in info
                    .decl
                    .variables(db)
                    .iter()
                    .filter(|v| v.is_program_located(db))
                {
                    let path =
                        crate::debug_symbols::join_path(db, inst.inst_name.text(db), var.name(db));
                    let key = global_key(db, *var);
                    if let Some((addr, ty)) = global_table.get(&key) {
                        crate::debug_symbols::collect_root(
                            db,
                            &path,
                            *addr,
                            ty,
                            false,
                            &mut symbols,
                            &mut array_syms,
                            &mut type_table,
                        );
                    } else if let Some(address) = var
                        .location(db)
                        .and_then(|dv| hir::hir_def::pous::variable::LocatedAddress::of(db, dv))
                        && let Some(part) = module
                            .located_map
                            .entries
                            .iter()
                            .find(|e| e.address == address.text.as_str())
                        && let (Some(of), Some(ty)) = (&part.part_of, part.ty)
                    {
                        // A part of a wider address has no cell of its own:
                        // it is the bits of its owner's.
                        symbols.push(debug_format::Symbol {
                            path,
                            address: part.addr,
                            size: part.size,
                            ty,
                            global: false,
                            named_type: None,
                            bits: Some(debug_format::SymBits {
                                shift: of.shift,
                                width: part.width,
                            }),
                        });
                    }
                }
            }
        }
    }
    // A variable VAR_CONFIG locates is a pointer in its instance, which the
    // walk above does not follow. Its symbol is the channel it points at,
    // under the instance's path and typed as declared.
    if let Some(sched) = &module.schedule {
        for loc in &config_locations {
            let Some(inst) = sched
                .tasks
                .iter()
                .flat_map(|t| t.programs.iter())
                .find(|p| p.inst_name.caseless(db) == loc.instance.caseless(db))
            else {
                continue;
            };
            let Some(var) = loc.members.last() else {
                continue;
            };
            let (owner, offset) = config_cell(db, &loc.address);
            let key = hir::hir_def::interned::identifier::Ident::new(db, owner.text.clone());
            let Some((cell, _)) = global_table.get(&key) else {
                continue;
            };
            let mut path = inst.inst_name.text(db).to_string();
            for member in &loc.members {
                path.push('.');
                path.push_str(member.name(db).text(db));
            }
            crate::debug_symbols::collect_root(
                db,
                &path,
                cell + offset,
                &super::lower_type::lower_spec(db, var.spec(db))?,
                false,
                &mut symbols,
                &mut array_syms,
                &mut type_table,
            );
        }
    }
    for (name, (addr, ty)) in &global_table {
        crate::debug_symbols::collect_root(
            db,
            name.text(db),
            *addr,
            ty,
            true,
            &mut symbols,
            &mut array_syms,
            &mut type_table,
        );
        crate::debug_symbols::collect_containers(
            db,
            name.text(db),
            *addr,
            ty,
            true,
            &mut containers,
        );
    }
    // A part of a wider address has no cell to walk: it is the bits of its
    // owner's, as the located map lists it.
    for part in &module.located_map.entries {
        let (Some(of), Some(ty)) = (&part.part_of, part.ty) else {
            continue;
        };
        symbols.push(debug_format::Symbol {
            path: part.name.clone(),
            address: part.addr,
            size: part.size,
            ty,
            global: true,
            named_type: None,
            bits: Some(debug_format::SymBits {
                shift: of.shift,
                width: part.width,
            }),
        });
    }
    symbols.sort_by(|a, b| a.path.cmp(&b.path));
    containers.sort_by(|a, b| a.path.cmp(&b.path));
    array_syms.sort_by(|a, b| a.path.cmp(&b.path));
    module.debug_symbols = crate::debug_symbols::DebugSymbols {
        version: crate::debug_symbols::DEBUG_SYMBOLS_VERSION,
        symbols,
        arrays: array_syms,
        types: type_table.into_entries(),
        containers,
    };

    // The per-field retain map from the same final addresses.
    module.retain_map = crate::retain_map::build_retain_map(
        db,
        module.schedule.as_ref(),
        &program_infos,
        &bands.retain_globals,
        &global_table
            .iter()
            .map(|(name, (_, ty))| (*name, ty.clone()))
            .collect(),
        bands.retain_base,
        bands.retain_size,
    )?;

    // Phase 4.6: the schedule travels as data in the `rk.schedule` section,
    // so a task keeps its name, priority and RESOURCE. Built after the
    // retain relocation.
    module.schedule_manifest = module.schedule.as_ref().map(|s| s.to_manifest(db));

    // Rewrite `Local(name)` -> `Global` for every global a body referenced,
    // now that addresses are final.
    for func in &mut module.functions {
        resolve_global_places(db, func, &global_table)?;
    }

    // `__init`: every scalar constant initializer at its final address. The
    // runtime calls it before restoring retain, so a RETAIN var's
    // initializer is its cold-start value.
    let init_stmts = collect_const_inits(
        db,
        config,
        all_programs,
        &global_table,
        &module.schedule,
        &program_infos,
        &string_pool,
    )?;
    if !init_stmts.is_empty() {
        let idx = module.functions.len() as u32 + module.extern_functions.len() as u32;
        let name = hir::hir_def::interned::identifier::Ident::new(
            db,
            compact_str::CompactString::from("__init"),
        );
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

    // The distinct source files, in body order, as `DebugLines::files`;
    // codegen resolves each statement's `file_url` against it.
    {
        let mut seen: FxHashSet<CompactString> = FxHashSet::default();
        let mut source_files: Vec<String> = Vec::new();
        for func in &module.functions {
            collect_source_files(&func.body, &mut seen, &mut source_files);
        }
        module.source_files = source_files;
    }

    Ok(module)
}

/// Lower an `{extern}` FUNCTION: `VAR_INPUT` are the params, scalar
/// `VAR_OUTPUT` the results in declaration order, the return type last
/// (E1502 refused the rest).
fn lower_extern_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    extern_decl: &hir::hir_def::pous::pragma::ExternPragma,
    index: u32,
) -> Result<MirExternFunction, LowerTypeError> {
    let mut params = Vec::new();
    let mut out_results = Vec::new();
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
            VariableKind::Output => {
                let ty = lower_type(db, var.spec(db).infer(db))?;
                out_results.push((var.name(db), ty));
            }
            _ => {}
        }
    }

    let return_type = func
        .return_type(db)
        .map(|spec| lower_type(db, spec.infer(db)))
        .transpose()?;

    Ok(MirExternFunction {
        name: super::naming::mir_function_symbol(db, func),
        index,
        module: extern_decl.module.clone(),
        import_name: extern_decl.name.clone(),
        params,
        out_results,
        return_type,
        linkage: crate::function::MirLinkage::Internal,
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
                selector,
                arms,
                else_body,
            } => {
                // The selector and a STRING label's test both hold pooled string
                // references.
                rebase_expr(selector, base);
                for arm in arms {
                    for pattern in &mut arm.patterns {
                        if let crate::stmt::MirCasePattern::Test(test) = pattern {
                            rebase_expr(test, base);
                        }
                    }
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

/// Collect the distinct source-file URLs referenced by `DebugTrap`
/// locations, in first-encounter order.
fn collect_source_files(
    stmts: &[crate::stmt::MirStmt],
    seen: &mut FxHashSet<CompactString>,
    source_files: &mut Vec<String>,
) {
    use crate::stmt::MirStmt;
    for stmt in stmts {
        match stmt {
            MirStmt::DebugTrap { location, .. } => {
                if seen.insert(location.file_url.clone()) {
                    source_files.push(location.file_url.to_string());
                }
            }
            MirStmt::If {
                then_body,
                else_ifs,
                else_body,
                ..
            } => {
                collect_source_files(then_body, seen, source_files);
                for (_, body) in else_ifs {
                    collect_source_files(body, seen, source_files);
                }
                if let Some(else_body) = else_body {
                    collect_source_files(else_body, seen, source_files);
                }
            }
            MirStmt::While { body, .. } | MirStmt::Repeat { body, .. } => {
                collect_source_files(body, seen, source_files);
            }
            MirStmt::For { body, .. } => collect_source_files(body, seen, source_files),
            MirStmt::Case {
                arms, else_body, ..
            } => {
                for arm in arms {
                    collect_source_files(&arm.body, seen, source_files);
                }
                if let Some(else_body) = else_body {
                    collect_source_files(else_body, seen, source_files);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// VAR_GLOBAL lowering
// ---------------------------------------------------------------------------

type GlobalTable<'db> =
    FxHashMap<hir::hir_def::interned::identifier::Ident, (u32, crate::types::MirType)>;

/// The address whose cell holds `address`, and where in it: the wider
/// address it is part of and its byte offset there, or itself at 0. Only an
/// address of a byte or more is given to a variable located by VAR_CONFIG
/// (E1424), so the offset is whole bytes.
fn config_cell(
    db: &dyn WorkspaceDataBase,
    address: &hir::hir_def::pous::variable::LocatedAddress,
) -> (hir::hir_def::pous::variable::LocatedAddress, u32) {
    match hir::hir_ty::index_graphs::located_view(db, address) {
        Some(view) => (view.owner, view.shift / 8),
        None => (address.clone(), 0),
    }
}

/// Where a variable a VAR_CONFIG path names sits in memory: its PROGRAM
/// instance's address plus each member's offset along the path. For a
/// variable VAR_CONFIG locates, that is its pointer slot. `None` for an
/// instance nothing schedules (E1412 has it).
fn config_member<'db>(
    db: &'db dyn WorkspaceDataBase,
    instance: hir::hir_def::interned::identifier::Ident,
    members: &[hir::hir_def::pous::variable::VariableDecl<'db>],
    schedule: &Option<crate::schedule::MirSchedule>,
    program_infos: &FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        crate::schedule::ProgramInfo<'db>,
    >,
) -> Option<u32> {
    let inst = schedule
        .as_ref()?
        .tasks
        .iter()
        .flat_map(|t| t.programs.iter())
        .find(|p| p.inst_name.caseless(db) == instance.caseless(db))?;
    let mut fields = &program_infos.get(&inst.prog_name)?.struct_type.fields;
    let mut address = inst.instance_addr;
    let (last, walked) = members.split_last()?;
    for member in walked {
        let field = fields
            .iter()
            .find(|f| f.name.caseless(db) == member.name(db).caseless(db))?;
        address += field.offset;
        let MirType::Struct(inner) = &field.ty else {
            return None;
        };
        fields = &inner.fields;
    }
    let field = fields
        .iter()
        .find(|f| f.name.caseless(db) == last.name(db).caseless(db))?;
    Some(address + field.offset)
}

/// The name a static cell is known by in the global table, the located map
/// and the debug symbols: a VAR_GLOBAL's own name, and `P.x` for a PROGRAM
/// `P`'s located VAR `x`, which two programs or a VAR_GLOBAL may otherwise
/// share.
pub(crate) fn global_key<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: hir::hir_def::pous::variable::VariableDecl<'db>,
) -> hir::hir_def::interned::identifier::Ident {
    use hir::HirNodeInfo;
    use hir::hir_def::scope::ScopeKind;
    if var.is_program_located(db)
        && let ScopeKind::Program(program) =
            hir::hir_def::semantic_index::get_scope(db, var.get_scope_id(db)).kind
    {
        return hir::hir_def::interned::identifier::Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}.{}",
                program.name(db).text(db),
                var.name(db).text(db)
            )),
        );
    }
    var.name(db)
}

/// The located VARs of the programs being lowered: static cells like the
/// located VAR_GLOBALs, whatever instances of the programs exist.
fn program_located<'a, 'db>(
    db: &'db dyn WorkspaceDataBase,
    programs: &'a [(&'a hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
) -> impl Iterator<Item = hir::hir_def::pous::variable::VariableDecl<'db>> + 'a
where
    'db: 'a,
{
    programs.iter().flat_map(move |(program, _)| {
        program
            .variables(db)
            .iter()
            .copied()
            .filter(move |v| v.is_program_located(db))
    })
}

/// Allocate a slot for every configuration VAR_GLOBAL and build the
/// symbol table; RETAIN globals go in the band, located (`AT %…`) ones in
/// the I/O band their area names.
fn build_global_table<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &[hir::hir_def::config::ConfigDecl<'db>],
    programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
    memory_layout: &mut MirMemoryLayout,
) -> Result<GlobalTable<'db>, LowerTypeError> {
    let mut table = GlobalTable::default();
    // One table across every fragment; a name declared twice is E0101.
    for config in config {
        // Application scope: a RESOURCE declares no variables of its own.
        for v in config.variables(db) {
            add_global(db, v, v.name(db), memory_layout, &mut table)?;
        }
    }
    for v in program_located(db, programs) {
        add_global(db, &v, global_key(db, v), memory_layout, &mut table)?;
    }
    Ok(table)
}

fn add_global<'db>(
    db: &'db dyn WorkspaceDataBase,
    v: &hir::hir_def::pous::variable::VariableDecl<'db>,
    key: hir::hir_def::interned::identifier::Ident,
    memory_layout: &mut MirMemoryLayout,
    table: &mut GlobalTable<'db>,
) -> Result<(), LowerTypeError> {
    // Located inside a wider address the workspace mentions, a global is no
    // cell of its own: it is that address's bits, and every access to it is
    // lowered as bits of the owner (`ExprLowerCtx::view`).
    if let Some(dv) = v.location(db)
        && let Some(address) = hir::hir_def::pous::variable::LocatedAddress::of(db, dv)
        && hir::hir_ty::index_graphs::located_view(db, &address).is_some()
    {
        return Ok(());
    }
    let ty = super::lower_type::lower_spec(db, v.spec(db))?;
    let size = ty.size_bytes();
    let align = ty.alignment();
    let addr = memory_layout.allocate(key, size, align, crate::memory::MirAllocKind::Variable);
    // A located global lives in its area's band instead of the globals band:
    // the host copies a whole direction at once, and a variable that is not
    // in that range would not travel with it.
    match located_entry(db, v, key, addr, size, align) {
        Some(entry) => memory_layout.record_located(entry),
        None => {
            // RETAIN globals are flagged so the globals band overlaps the
            // retain band on them.
            let retain = v.qualifier(db).contains(hir::Qualifier::RETAIN);
            memory_layout.record_global(key, addr, size, align, retain);
        }
    }
    table.insert(key, (addr, ty));
    Ok(())
}

/// The band entry for a located VAR_GLOBAL, or `None` when the address names
/// no band — an unknown area letter, a missing or unknown width letter, or
/// the incomplete `%I*`, each of which `rk check` refuses (E1417) before
/// lowering ever runs. MIR only has to not invent a band for them.
fn located_entry<'db>(
    db: &'db dyn WorkspaceDataBase,
    v: &hir::hir_def::pous::variable::VariableDecl<'db>,
    name: hir::hir_def::interned::identifier::Ident,
    address: u32,
    size: u32,
    align: u32,
) -> Option<crate::memory::LocatedEntry> {
    let dv = v.location(db)?;
    Some(crate::memory::LocatedEntry {
        name,
        address_text: dv.to_address(db),
        located: hir::hir_def::pous::variable::LocatedAddress::of(db, dv)?,
        // Only `%M` reaches here with it set: E1420 refuses RETAIN on an
        // input or output image.
        retain: v.qualifier(db).contains(hir::Qualifier::RETAIN),
        address,
        size,
        align,
    })
}

/// The located map the module carries: every located variable at its FINAL
/// address, paired with the address a host binds a channel to. The band
/// exports say where the three areas are; this says which cell is which
/// inside them.
fn build_located_map<'db>(
    db: &'db dyn WorkspaceDataBase,
    located: &[crate::memory::LocatedEntry],
    globals: &GlobalTable<'db>,
    config_locations: &[&hir::hir_ty::config::ConfigLocation<'db>],
) -> debug_format::LocatedMap {
    use hir::hir_def::pous::variable::LocationArea;
    let area = |a: LocationArea| match a {
        LocationArea::Input => debug_format::LocatedArea::Input,
        LocationArea::Output => debug_format::LocatedArea::Output,
        LocationArea::Marker => debug_format::LocatedArea::Marker,
    };
    // An address no declaration names but VAR_CONFIG gives a variable is
    // typed as that variable is declared, not by its width.
    let configured_type = |address: &hir::hir_def::pous::variable::LocatedAddress| {
        config_locations
            .iter()
            .find(|loc| loc.address == *address)
            .and_then(|loc| loc.members.last())
            .and_then(|var| super::lower_type::lower_spec(db, var.spec(db)).ok())
            .and_then(|ty| crate::debug_symbols::scalar_sym_ty(&ty))
    };
    let mut entries: Vec<debug_format::LocatedVar> = located
        .iter()
        .map(|e| debug_format::LocatedVar {
            address: e.address_text.clone(),
            name: e.name.text(db).to_string(),
            area: area(e.located.area),
            path: e.located.levels.clone(),
            width: u16::from(e.located.width),
            addr: e.address,
            size: e.size,
            ty: hir::hir_ty::index_graphs::located_declaration(db, &e.located)
                .is_none()
                .then(|| configured_type(&e.located))
                .flatten()
                .or_else(|| {
                    globals
                        .get(&e.name)
                        .and_then(|(_, ty)| crate::debug_symbols::scalar_sym_ty(ty))
                }),
            part_of: None,
        })
        .collect();

    // The parts: addresses stored inside a wider one, with no cell of their
    // own. Each is listed at its owner's cell with the bits it is, so a host
    // finds every address the program names, part or not, the same way; an
    // address VAR_CONFIG gives an instance variable included.
    let mut parts: Vec<debug_format::LocatedVar> = Vec::new();
    {
        let mentioned = hir::hir_ty::index_graphs::located_by_file(db).flat_map(|m| m.keys());
        for address in mentioned.chain(config_locations.iter().map(|loc| &loc.address)) {
            if parts.iter().any(|p| p.address == address.text.as_str()) {
                continue;
            }
            let Some(view) = hir::hir_ty::index_graphs::located_view(db, address) else {
                continue;
            };
            let Some(owner) = entries
                .iter()
                .find(|cell| cell.address.eq_ignore_ascii_case(&view.owner.text))
            else {
                continue;
            };
            // Under the name it was declared with, if it was; the address
            // stands in for a bare one, as it does for a bare cell.
            let declared = hir::hir_ty::index_graphs::located_declaration(db, address);
            let (name, ty) = match declared {
                Some(v) => (
                    global_key(db, v).text(db).to_string(),
                    super::lower_type::lower_spec(db, v.spec(db))
                        .ok()
                        .and_then(|ty| crate::debug_symbols::scalar_sym_ty(&ty)),
                ),
                None => (
                    address.text.to_string(),
                    configured_type(address).or(Some(crate::debug_symbols::sym_type_of(
                        crate::located::width_elementary(address.width),
                    ))),
                ),
            };
            parts.push(debug_format::LocatedVar {
                address: address.text.to_string(),
                name,
                area: area(address.area),
                path: address.levels.clone(),
                width: u16::from(address.width),
                addr: owner.addr,
                size: owner.size,
                ty,
                part_of: Some(debug_format::LocatedPart {
                    owner: owner.address.clone(),
                    shift: view.shift as u16,
                }),
            });
        }
    }
    entries.append(&mut parts);
    debug_format::LocatedMap::new(entries)
}

/// Fill in the address and type of each global a body referenced by
/// name, now that the layout is final.
fn resolve_global_places<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: &mut crate::function::MirFunction,
    globals: &GlobalTable<'db>,
) -> Result<(), LowerTypeError> {
    let mut missing = Vec::new();
    for stmt in &mut func.body {
        rewrite_globals_stmt(stmt, globals, &mut missing);
    }
    match missing.first() {
        Some((name, _)) => Err(LowerTypeError::UnsupportedType(format!(
            "no storage was allocated for global '{}'",
            name.text(db)
        ))),
        None => Ok(()),
    }
}

/// Give every address a body wrote BARE (`%IW4`) a cell of its own.
///
/// Such an address declares nothing, so there is no VariableDecl to allocate
/// against — lowering emits it as a global named by the address itself, and
/// this is where that name gets storage. Two mentions of the same address, in
/// any two bodies, are one cell; a VAR_GLOBAL declared `AT` the same address
/// is that same cell too, rather than a second one the host would have to
/// bind twice.
///
/// It runs before the bands are carved, and it reuses the global rewriter to
/// find the names rather than walking the bodies a second way: a name the
/// table does not hold comes back as missing, and one of the addresses the
/// workspace's files mention is the only kind of missing name that is not an
/// error. Each is taken as HIR decoded it, never read again from its text.
fn synthesize_bare_addresses<'db>(
    db: &'db dyn WorkspaceDataBase,
    functions: &mut [crate::function::MirFunction],
    config_cells: &[hir::hir_def::pous::variable::LocatedAddress],
    memory_layout: &mut MirMemoryLayout,
    table: &mut GlobalTable<'db>,
) -> Result<(), LowerTypeError> {
    use hir::hir_def::pous::variable::LocatedAddress;
    // Every address the workspace's files mention, by the text a bare one is
    // lowered under.
    let mentioned: FxHashMap<&str, &LocatedAddress> =
        hir::hir_ty::index_graphs::located_by_file(db)
            .flat_map(|m| m.keys())
            .map(|a| (a.text.as_str(), a))
            .collect();
    // Keyed by the address text so the allocation order is the addresses'
    // own and not the order the bodies happen to mention them in.
    let mut wanted: std::collections::BTreeMap<
        String,
        (
            hir::hir_def::interned::identifier::Ident,
            crate::types::MirType,
            LocatedAddress,
        ),
    > = std::collections::BTreeMap::new();
    for func in functions.iter_mut() {
        let mut missing = Vec::new();
        for stmt in &mut func.body {
            rewrite_globals_stmt(stmt, table, &mut missing);
        }
        for (name, ty) in missing {
            if let Some(address) = mentioned.get(name.text(db).as_str()) {
                wanted
                    .entry(address.text.to_string())
                    .or_insert((name, ty, (*address).clone()));
            }
            // Anything else is a global that really is missing; the pass that
            // runs once the layout is final reports it.
        }
    }
    for address in config_cells {
        let name = hir::hir_def::interned::identifier::Ident::new(db, address.text.clone());
        if table.contains_key(&name) {
            continue;
        }
        wanted.entry(address.text.to_string()).or_insert((
            name,
            MirType::Elementary(crate::located::width_elementary(address.width)),
            address.clone(),
        ));
    }

    for (text, (name, ty, located)) in wanted {
        // A declaration already bound this address, so the bare mention is
        // the same storage under a second name.
        if let Some(declared) = memory_layout
            .located_allocations
            .iter()
            .find(|e| e.located == located)
            && let Some((address, declared_ty)) = table.get(&declared.name).cloned()
        {
            table.insert(name, (address, declared_ty));
            continue;
        }
        let size = ty.size_bytes();
        let align = ty.alignment();
        let address =
            memory_layout.allocate(name, size, align, crate::memory::MirAllocKind::Variable);
        memory_layout.record_located(crate::memory::LocatedEntry {
            name,
            address_text: text,
            located,
            // A bare address has no declaration, so nothing can have asked
            // for it to persist.
            retain: false,
            address,
            size,
            align,
        });
        table.insert(name, (address, ty));
    }
    Ok(())
}

fn rewrite_globals_place(
    place: &mut crate::expr::MirPlace,
    globals: &GlobalTable<'_>,
    missing: &mut Vec<(
        hir::hir_def::interned::identifier::Ident,
        crate::types::MirType,
    )>,
) {
    use crate::expr::MirPlace;
    match place {
        MirPlace::Global {
            name: Some(name),
            address,
            ty,
        } => match globals.get(name) {
            Some((addr, global_ty)) => {
                *address = *addr;
                *ty = global_ty.clone();
            }
            None => missing.push((*name, ty.clone())),
        },
        MirPlace::Local(_) => {}
        MirPlace::Field { base, .. } | MirPlace::Deref { base, .. } => {
            rewrite_globals_place(base, globals, missing);
        }
        MirPlace::Index { base, index, .. } => {
            rewrite_globals_place(base, globals, missing);
            rewrite_globals_expr(index, globals, missing);
        }
        MirPlace::ThisField { .. } | MirPlace::Global { name: None, .. } => {}
    }
}

fn rewrite_globals_expr(
    expr: &mut crate::expr::MirExpr,
    globals: &GlobalTable<'_>,
    missing: &mut Vec<(
        hir::hir_def::interned::identifier::Ident,
        crate::types::MirType,
    )>,
) {
    use crate::expr::MirExpr;
    match expr {
        MirExpr::Load(place, _) | MirExpr::AddrOf(place) => {
            rewrite_globals_place(place, globals, missing);
        }
        MirExpr::CopyIntoScratch { src, .. } => {
            // The scratch is always a true local; only the source expression may
            // name a global.
            rewrite_globals_expr(src, globals, missing);
        }
        MirExpr::BinOp { lhs, rhs, .. } => {
            rewrite_globals_expr(lhs, globals, missing);
            rewrite_globals_expr(rhs, globals, missing);
        }
        MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
            rewrite_globals_expr(expr, globals, missing);
        }
        MirExpr::Call(call) => {
            for arg in &mut call.args {
                rewrite_globals_expr(&mut arg.value, globals, missing);
            }
            for bind in &mut call.output_bindings {
                rewrite_globals_place(&mut bind.target, globals, missing);
                rewrite_globals_expr(&mut bind.value, globals, missing);
            }
            for bind in &mut call.extern_results {
                if let Some(dest) = &mut bind.dest {
                    rewrite_globals_place(dest, globals, missing);
                }
            }
        }
        MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => {}
    }
}

fn rewrite_globals_stmt(
    stmt: &mut crate::stmt::MirStmt,
    globals: &GlobalTable<'_>,
    missing: &mut Vec<(
        hir::hir_def::interned::identifier::Ident,
        crate::types::MirType,
    )>,
) {
    use crate::stmt::MirStmt;
    match stmt {
        MirStmt::Assign { target, value } => {
            rewrite_globals_place(target, globals, missing);
            rewrite_globals_expr(value, globals, missing);
        }
        MirStmt::Call(call) => {
            for arg in &mut call.args {
                rewrite_globals_expr(&mut arg.value, globals, missing);
            }
            for binding in &mut call.output_bindings {
                rewrite_globals_place(&mut binding.target, globals, missing);
                rewrite_globals_expr(&mut binding.value, globals, missing);
            }
        }
        MirStmt::FbCall {
            instance,
            input_writes,
            output_reads,
            ..
        } => {
            rewrite_globals_place(instance, globals, missing);
            for (_, value, _) in input_writes {
                rewrite_globals_expr(value, globals, missing);
            }
            for (_, target, _, _) in output_reads {
                rewrite_globals_place(target, globals, missing);
            }
        }
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            rewrite_globals_expr(condition, globals, missing);
            rewrite_globals_body(then_body, globals, missing);
            for (cond, body) in else_ifs {
                rewrite_globals_expr(cond, globals, missing);
                rewrite_globals_body(body, globals, missing);
            }
            if let Some(body) = else_body {
                rewrite_globals_body(body, globals, missing);
            }
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            rewrite_globals_expr(selector, globals, missing);
            for arm in arms {
                rewrite_globals_body(&mut arm.body, globals, missing);
            }
            if let Some(body) = else_body {
                rewrite_globals_body(body, globals, missing);
            }
        }
        MirStmt::For {
            control,
            start,
            end,
            step,
            body,
            ..
        } => {
            // The counter may be a global too, or bytes of a located cell.
            rewrite_globals_place(control, globals, missing);
            rewrite_globals_expr(start, globals, missing);
            rewrite_globals_expr(end, globals, missing);
            rewrite_globals_expr(step, globals, missing);
            rewrite_globals_body(body, globals, missing);
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            rewrite_globals_expr(condition, globals, missing);
            rewrite_globals_body(body, globals, missing);
        }
        MirStmt::Raise { message } => rewrite_globals_expr(message, globals, missing),
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
    missing: &mut Vec<(
        hir::hir_def::interned::identifier::Ident,
        crate::types::MirType,
    )>,
) {
    for stmt in body {
        rewrite_globals_stmt(stmt, globals, missing);
    }
}

/// The `Assign { Global{addr}, <const> }` statements that run once at
/// startup: configuration VAR_GLOBALs and program-instance fields.
fn collect_const_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &[hir::hir_def::config::ConfigDecl<'db>],
    programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
    global_table: &GlobalTable<'db>,
    schedule: &Option<crate::schedule::MirSchedule>,
    program_infos: &FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        crate::schedule::ProgramInfo<'db>,
    >,
    string_pool: &std::rc::Rc<std::cell::RefCell<crate::lower::lower_expr::StringPool>>,
) -> Result<Vec<crate::stmt::MirStmt>, LowerTypeError> {
    let mut stmts = Vec::new();

    // Every fragment's VAR_GLOBALs, then every PROGRAM's located VARs: both
    // are static cells, initialized once whatever instances exist.
    let statics: Vec<_> = config
        .iter()
        .flat_map(|c| c.variables(db).iter().map(|v| (*v, v.name(db))))
        .chain(program_located(db, programs).map(|v| (v, global_key(db, v))))
        .collect();
    for (v, key) in statics {
        let Some((addr, ty)) = global_table.get(&key) else {
            continue;
        };
        // TYPE defaults first; a declaration init overlays by store order.
        super::lower_func::lower_type_default_inits(
            db,
            super::lower_func::InitTarget::Static { base: *addr },
            ty,
            v.spec(db).infer(db),
            string_pool,
            &mut stmts,
        )?;
        if let Some(init) = v.init(db) {
            super::lower_func::lower_resolved_init_into(
                db,
                *addr,
                ty,
                init,
                &mut stmts,
                string_pool,
            )?;
        } else {
            // A global FB instance is initialized from its type's members;
            // an array of them, once per element.
            super::lower_func::lower_declared_instance_inits(
                db,
                super::lower_func::InitTarget::Static { base: *addr },
                ty,
                v.spec(db).infer(db),
                string_pool,
                &mut stmts,
            )?;
        }
    }

    // Program instance fields.
    if let Some(sched) = schedule {
        for task in &sched.tasks {
            for inst in &task.programs {
                // Same invariant as the schedule and debug-symbol walks.
                let Some(info) = program_infos.get(&inst.prog_name) else {
                    return Err(LowerTypeError::UnsupportedType(format!(
                        "scheduled instance '{}' names program '{}' with no lowered info",
                        inst.inst_name.text(db),
                        inst.prog_name.text(db)
                    )));
                };
                for field in &info.struct_type.fields {
                    // A pointer VAR_CONFIG binds, below; its value goes to
                    // the channel.
                    if field.by_ref {
                        continue;
                    }
                    let Some(var) = info
                        .decl
                        .variables(db)
                        .iter()
                        .find(|v| v.name(db).caseless(db) == field.name.caseless(db))
                    else {
                        continue;
                    };
                    let addr = inst.instance_addr + field.offset;
                    // TYPE defaults first, same overlay rule as everywhere.
                    super::lower_func::lower_type_default_inits(
                        db,
                        super::lower_func::InitTarget::Static { base: addr },
                        &field.ty,
                        var.spec(db).infer(db),
                        string_pool,
                        &mut stmts,
                    )?;
                    if let Some(init) = var.init(db) {
                        super::lower_func::lower_resolved_init_into(
                            db,
                            addr,
                            &field.ty,
                            init,
                            &mut stmts,
                            string_pool,
                        )?;
                    } else {
                        // An FB instance held by a PROGRAM gets its type's
                        // member initializers, same as one held by a FUNCTION.
                        super::lower_func::lower_declared_instance_inits(
                            db,
                            super::lower_func::InitTarget::Static { base: addr },
                            &field.ty,
                            var.spec(db).infer(db),
                            string_pool,
                            &mut stmts,
                        )?;
                    }
                }
            }
        }
    }

    // Each variable VAR_CONFIG locates: its instance's pointer slot is given
    // the address of its channel, and its type's default, if it has one
    // (`TYPE Speed : INT := 5`), goes to the channel. The grammar gives the
    // variable no initial value of its own. A declaration at the channel, or
    // at the cell the channel is part of, already gave the cell its initial
    // value, and that one stands.
    for c in config {
        for loc in &hir::hir_ty::config::infer_config_result(db, *c).locations {
            let Some(slot) = config_member(db, loc.instance, &loc.members, schedule, program_infos)
            else {
                continue;
            };
            let (owner, offset) = config_cell(db, &loc.address);
            let key = hir::hir_def::interned::identifier::Ident::new(db, owner.text.clone());
            let Some((cell, _)) = global_table.get(&key) else {
                return Err(LowerTypeError::UnsupportedType(format!(
                    "'{}' was given no cell for VAR_CONFIG to point at",
                    owner.text
                )));
            };
            let channel = cell + offset;
            stmts.push(crate::stmt::MirStmt::MemStore {
                offset: slot,
                value: crate::expr::MirConstant::I32(channel as i32),
            });
            let declared = hir::hir_ty::index_graphs::located_declaration(db, &loc.address)
                .or_else(|| hir::hir_ty::index_graphs::located_declaration(db, &owner));
            if let Some(var) = loc.members.last()
                && declared.is_none()
            {
                let ty = super::lower_type::lower_spec(db, var.spec(db))?;
                super::lower_func::lower_type_default_inits(
                    db,
                    super::lower_func::InitTarget::Static { base: channel },
                    &ty,
                    var.spec(db).infer(db),
                    string_pool,
                    &mut stmts,
                )?;
            }
        }
    }

    // Each VAR_CONFIG value: a variable's starting value in one instance,
    // written after the instance's own initializers, which it overrides. A
    // value for a variable VAR_CONFIG locates goes to its channel, over the
    // type default the binding above wrote there.
    for c in config {
        for value in &hir::hir_ty::config::infer_config_result(db, *c).values {
            let Some(var) = value.members.last() else {
                continue;
            };
            let base = match &value.channel {
                Some(address) => {
                    let (owner, offset) = config_cell(db, address);
                    let key =
                        hir::hir_def::interned::identifier::Ident::new(db, owner.text.clone());
                    let Some((cell, _)) = global_table.get(&key) else {
                        return Err(LowerTypeError::UnsupportedType(format!(
                            "'{}' was given no cell for a VAR_CONFIG value",
                            owner.text
                        )));
                    };
                    cell + offset
                }
                None => {
                    let Some(address) =
                        config_member(db, value.instance, &value.members, schedule, program_infos)
                    else {
                        continue;
                    };
                    address
                }
            };
            let ty = super::lower_type::lower_spec(db, var.spec(db))?;
            super::lower_func::lower_resolved_init_into(
                db,
                base,
                &ty,
                value.init,
                &mut stmts,
                string_pool,
            )?;
        }
    }
    Ok(stmts)
}
