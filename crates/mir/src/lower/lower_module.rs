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
        lower_type::{LowerTypeError, lower_fb_type, lower_type},
        monomorphize::{AnyFunctionInfo, detect_any_function, monomorphize},
    },
    memory::MirMemoryLayout,
    types::MirType,
};

/// Lower multiple HIR semantic indices (from multiple files) into a single MirModule.
pub fn lower_modules<'db>(
    db: &'db dyn WorkspaceDataBase,
    indices: &[&SemanticIndex<'db>],
) -> Result<MirModule, LowerTypeError> {
    // Collect all POUs and programs from all files
    let mut all_pous = Vec::new();
    let mut all_programs = Vec::new();
    for index in indices {
        all_pous.extend(index.global_pous.iter());
        all_programs.extend(index.programs.iter());
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
        &index.global_pous.iter().collect::<Vec<_>>(),
        &index.programs.iter().collect::<Vec<_>>(),
    )
}

fn lower_module_from_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_pous: &[&Pou<'db>],
    all_programs: &[&hir::hir_def::program::ProgramDecl<'db>],
) -> Result<MirModule, LowerTypeError> {
    let mut functions = Vec::new();
    let mut extern_functions = Vec::new();
    let mut function_indices = FxHashMap::default();
    let type_indices = FxHashMap::default();
    let mut instance_types = Vec::new();
    let mut memory_layout = MirMemoryLayout::new();
    let mut next_fn_idx: u32 = 0;

    // Collect ANY_* functions for deferred monomorphization
    let mut any_functions: Vec<AnyFunctionInfo<'db>> = Vec::new();

    // Phase 1: Process imports first (extern functions get lower indices)
    for pou in all_pous.iter() {
        if let Pou::Function(func) = pou {
            let extern_decl = find_extern_decl(db, *func);
            if extern_decl.is_none() {
                continue;
            }
            let extern_decl = extern_decl.unwrap();

            // Check if ANY_* — defer to monomorphization
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
    for pou in all_pous.iter() {
        match pou {
            Pou::Function(func) => {
                // Skip already-processed externs and ANY_* functions
                if function_indices.contains_key(&func.name(db)) {
                    continue;
                }
                if any_functions.iter().any(|a| a.func.name(db) == func.name(db)) {
                    continue;
                }

                // Check if extern (already handled in phase 1)
                let is_extern = func.statements(db).iter().any(|s| {
                    matches!(s.stmt(db), StmtKind::ExternPragma(_))
                });
                if is_extern {
                    continue;
                }

                // Check if non-extern ANY_* (deferred)
                if let Some(any_info) = detect_any_function(db, *func) {
                    any_functions.push(any_info);
                    continue;
                }

                let mir_func = lower_function(db, *func, next_fn_idx, &mut memory_layout)?;
                function_indices.insert(func.name(db), next_fn_idx);
                next_fn_idx += 1;
                functions.push(mir_func);
            }

            Pou::FunctionBlock(fb) => {
                // Build instance type
                let fb_mir_type = lower_fb_type(db, *fb)?;
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
                let method_funcs =
                    lower_function_block(db, *fb, next_fn_idx, &mut memory_layout)?;
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
                let method_funcs =
                    lower_class(db, *class, next_fn_idx, &mut memory_layout)?;
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
    for program in all_programs.iter() {
        let mir_func = lower_program(db, **program, next_fn_idx, &mut memory_layout)?;
        function_indices.insert(program.name(db), next_fn_idx);
        next_fn_idx += 1;
        functions.push(mir_func);
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
    };

    // Phase 4: Monomorphization — discovers call sites, generates concrete copies
    if !any_functions.is_empty() {
        monomorphize(db, &mut module, &any_functions, &mut MirMemoryLayout::new())?;
    }

    Ok(module)
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
