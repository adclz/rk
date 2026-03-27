use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::{
    expressions::statement::StmtKind,
    pous::{function::Function, function_block::FunctionBlock, pou::Pou, variable::VariableKind},
    semantic_index::SemanticIndex,
};
use hir::hir_ty::infer::Infer;
use rustc_hash::FxHashMap;

use crate::{
    MirInstanceField, MirInstanceType, MirModule,
    function::{MirExternFunction, MirParam, MirParamKind},
    lower::{
        lower_func::{lower_class, lower_function, lower_function_block, lower_program},
        lower_type::{LowerTypeError, lower_fb_type, lower_fb_type_with_subs, lower_type},
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
    for index in indices {
        all_pous.extend(index.global_pous.iter().map(|p| (p, None)));
        all_programs.extend(index.programs.iter().map(|p| (p, None)));
        for ns in index.namespaces.iter() {
            collect_namespace_pous(db, ns, &mut all_pous);
        }
    }
    lower_module_from_pous(db, &all_pous, &all_programs)
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
    )
}

fn lower_module_from_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    all_programs: &[(&hir::hir_def::program::ProgramDecl<'db>, Option<String>)],
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

    // Pre-compute ANY_* substitutions for all FBs.
    // Key: FB name, Value: map of variable name → concrete ElementarySpec.
    let mut all_fb_subs: FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        FxHashMap<hir::hir_def::interned::identifier::Ident, hir::hir_def::expressions::spec::ElementarySpec>,
    > = FxHashMap::default();
    for (pou, _) in all_pous.iter() {
        if let Pou::FunctionBlock(fb) = pou {
            let has_any = fb.variables(db).iter().any(|v| {
                let ty = v.spec(db).infer(db).normalize(db);
                matches!(ty, hir::hir_ty::ty::Type::Elementary(e) if e.is_any())
            });
            if has_any {
                let subs = collect_fb_any_subs(db, all_pous, *fb);
                if !subs.is_empty() {
                    all_fb_subs.insert(fb.name(db), subs);
                }
            }
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
            function_indices.insert(func.name(db), next_fn_idx);
            next_fn_idx += 1;
            extern_functions.push(mir_ext);
        }
    }

    // Phase 2: Process local functions
    for (pou, ns_prefix) in all_pous.iter() {
        match pou {
            Pou::Function(func) => {
                // Skip already-processed externs and ANY_* functions
                if function_indices.contains_key(&func.name(db)) {
                    continue;
                }
                if any_functions
                    .iter()
                    .any(|a| a.func.name(db) == func.name(db))
                {
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
                    if let Ok(mut mir_func) = lower_wasm_intrinsic(db, *func, &wasm_decl, next_fn_idx) {
                        mir_func.export_name =
                            make_export_name(ns_prefix, func.name(db).text(db));

                        if func.is_test(db) {
                            let export_name = mir_func
                                .export_name
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| func.name(db).text(db).to_string());
                            let cases = build_test_cases(db, *func, &export_name);
                            test_entries.push(crate::test_manifest::TestEntry {
                                path: export_name.clone(),
                                export: export_name,
                                cases,
                            });
                        }

                        function_indices.insert(func.name(db), next_fn_idx);
                        next_fn_idx += 1;
                        functions.push(mir_func);
                    }
                    continue;
                }

                // Skip stub functions (no body, no extern pragma)
                if func.statements(db).is_empty() {
                    continue;
                }

                let mut mir_func = lower_function(
                    db,
                    *func,
                    next_fn_idx,
                    &mut memory_layout,
                    string_pool.clone(),
                    &all_fb_subs,
                )?;
                mir_func.export_name = make_export_name(ns_prefix, func.name(db).text(db));
                function_indices.insert(func.name(db), next_fn_idx);
                next_fn_idx += 1;

                // Collect test entry if marked with {test}
                if func.is_test(db) {
                    let export_name = mir_func
                        .export_name
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| func.name(db).text(db).to_string());
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
                let has_any = fb.variables(db).iter().any(|v| {
                    let ty = v.spec(db).infer(db).normalize(db);
                    matches!(ty, hir::hir_ty::ty::Type::Elementary(e) if e.is_any())
                });

                // For FBs with ANY_* vars, collect resolutions from all function bodies
                let any_subs = if has_any {
                    collect_fb_any_subs(db, all_pous, *fb)
                } else {
                    FxHashMap::default()
                };

                // Build instance type (with substitutions if ANY)
                let fb_mir_type = lower_fb_type_with_subs(db, *fb, &any_subs)?;
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
                        name: fb.name(db),
                        fields: inst_fields,
                        size: struct_type.size,
                        align: struct_type.align,
                    });
                }

                // Lower methods
                let method_funcs = lower_function_block(
                    db,
                    *fb,
                    next_fn_idx,
                    &mut memory_layout,
                    string_pool.clone(),
                    &any_subs,
                )?;
                for mf in method_funcs {
                    function_indices.insert(mf.name, mf.index);
                    next_fn_idx += 1;
                    functions.push(mf);
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
                        name: class.name(db),
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

    // Phase 3: Process programs
    for (program, ns_prefix) in all_programs.iter() {
        let mut mir_func = lower_program(
            db,
            **program,
            next_fn_idx,
            &mut memory_layout,
            string_pool.clone(),
        )?;
        mir_func.export_name = make_export_name(ns_prefix, program.name(db).text(db));

        if program.is_test(db) {
            let export_name = mir_func
                .export_name
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| program.name(db).text(db).to_string());
            test_entries.push(crate::test_manifest::TestEntry {
                path: export_name.clone(),
                export: export_name,
                cases: vec![],
            });
        }

        function_indices.insert(program.name(db), next_fn_idx);
        next_fn_idx += 1;
        functions.push(mir_func);
    }

    // Sort test entries by path for deterministic output
    test_entries.sort_by(|a, b| a.path.cmp(&b.path));

    let static_mem_end = memory_layout.total_size();

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
    use hir::hir_def::expressions::expression::{
        ExprKind, ParamAssignKind, PrimaryExpr,
    };

    func.cases(db)
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
        name: func.name(db),
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
) -> Result<crate::function::MirFunction, LowerTypeError> {
    use crate::expr::*;
    use crate::function::*;
    use crate::stmt::*;
    use hir::hir_def::pous::variable::VariableKind;

    // Parse the instruction to determine from/to types
    let _instruction = wasm_decl.instruction.as_str();

    // Build params
    let mut params = Vec::new();
    let mut param_name = None;
    let mut param_elem = None;
    for var in func.variables(db) {
        if matches!(var.kind(db), VariableKind::Input) {
            let ty = lower_type(db, var.spec(db).infer(db))?;
            if let MirType::Elementary(e) = &ty {
                param_elem = Some(*e);
            }
            param_name = Some(var.name(db));
            params.push(MirParam {
                name: var.name(db),
                ty,
                kind: MirParamKind::Input,
            });
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

    // Return local - needed so the local map has an entry for the return variable
    let mut locals = Vec::new();
    let return_local_idx = params.len() as u32; // after all params
    if let Some(ref ret_ty) = return_type {
        locals.push(crate::function::MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: crate::function::MirLocalKind::Var,
            storage: MirStorage::Scalar {
                local_index: return_local_idx,
            },
        });
    }

    // Build body: result := cast(param)
    let mut body = Vec::new();
    if let (Some(p_name), Some(from), Some(to)) = (param_name, param_elem, return_elem) {
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
        body.push(MirStmt::Assign {
            target: MirPlace::Local(func.name(db)),
            value: cast_expr,
        });
    }

    Ok(MirFunction {
        name: func.name(db),
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

/// Collect ANY_* type substitutions for a function block from all call sites.
/// Walks all function bodies and collects `fb_any_resolutions` from their inference results.
fn collect_fb_any_subs<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[(&Pou<'db>, Option<String>)],
    target_fb: FunctionBlock<'db>,
) -> FxHashMap<hir::hir_def::interned::identifier::Ident, hir::hir_def::expressions::spec::ElementarySpec> {
    use hir::hir_ty::body::infer_body;

    let mut subs = FxHashMap::default();
    let target_name = target_fb.name(db);

    for (pou, _) in all_pous {
        let scope = match pou {
            Pou::Function(f) => f.scope_id(db),
            Pou::FunctionBlock(fb) => fb.scope_id(db),
            _ => continue,
        };

        let body = infer_body(db, scope);
        for ((var_decl, field_name), concrete) in &body.fb_any_resolutions {
            // Check if this variable's type matches the target FB
            let var_type = var_decl.spec(db).infer(db).normalize(db);
            let is_target = match var_type {
                hir::hir_ty::ty::Type::FunctionBlock(fb) => fb.name(db) == target_name,
                _ => false,
            };
            if is_target {
                subs.insert(*field_name, *concrete);
            }
        }
    }

    subs
}

/// Rebase all StringLiteral offsets in MIR statements by adding `base` to each offset.
fn rebase_string_offsets(stmts: &mut [crate::stmt::MirStmt], base: u32) {
    use crate::expr::MirExpr;
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
            MirStmt::If { condition, then_body, else_body, .. } => {
                rebase_expr(condition, base);
                rebase_string_offsets(then_body, base);
                if let Some(else_body) = else_body {
                    rebase_string_offsets(else_body, base);
                }
            }
            MirStmt::While { condition, body, .. } => {
                rebase_expr(condition, base);
                rebase_string_offsets(body, base);
            }
            MirStmt::For { body, .. } => {
                rebase_string_offsets(body, base);
            }
            MirStmt::Repeat { condition, body, .. } => {
                rebase_expr(condition, base);
                rebase_string_offsets(body, base);
            }
            MirStmt::Case { arms, else_body, .. } => {
                for arm in arms {
                    rebase_string_offsets(&mut arm.body, base);
                }
                if let Some(else_body) = else_body {
                    rebase_string_offsets(else_body, base);
                }
            }
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
