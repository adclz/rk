//! Wraps a core WASM module into a WASM Component using wasm-encoder's ComponentBuilder.
//!
//! Flow:
//! 1. Define component-level types for each import/export
//! 2. Import component functions + canon lower them to core functions
//! 3. Bundle lowered imports into a core instance
//! 4. Embed core module + instantiate with the import instance
//! 5. Alias core exports + canon lift them to component functions
//! 6. Export the lifted functions

use db::WorkspaceDataBase;
use mir::types::{MirElementary, MirType};
use mir::MirModule;
use wasm_encoder::{
    CanonicalOption, ComponentBuilder, ComponentExportKind, ComponentTypeRef, ComponentValType,
    ExportKind, ModuleArg, PrimitiveValType,
};

/// Convert MIR type to component PrimitiveValType.
fn mir_to_prim(ty: &MirType) -> PrimitiveValType {
    match ty {
        MirType::Elementary(e) => match e {
            MirElementary::Bool => PrimitiveValType::Bool,
            MirElementary::SInt | MirElementary::Int | MirElementary::DInt => {
                PrimitiveValType::S32
            }
            MirElementary::USInt | MirElementary::UInt | MirElementary::UDInt => {
                PrimitiveValType::U32
            }
            MirElementary::Byte | MirElementary::Word | MirElementary::DWord => {
                PrimitiveValType::U32
            }
            MirElementary::LInt => PrimitiveValType::S64,
            MirElementary::ULInt | MirElementary::LWord => PrimitiveValType::U64,
            MirElementary::Real => PrimitiveValType::F32,
            MirElementary::LReal => PrimitiveValType::F64,
            MirElementary::Time | MirElementary::LTime | MirElementary::Date
            | MirElementary::LDate | MirElementary::Tod | MirElementary::LTod
            | MirElementary::DateAndTime | MirElementary::LDateTime => PrimitiveValType::S64,
            MirElementary::Char | MirElementary::WChar => PrimitiveValType::Char,
        },
        MirType::Pointer(_) => PrimitiveValType::U32,
        // Strings are passed as (ptr, len) in core WASM — at the component level
        // we expose them as u32 pairs until proper canonical string ABI is wired up.
        MirType::String(_) => PrimitiveValType::U32,
        _ => PrimitiveValType::S32,
    }
}

/// Convert name to kebab-case for WIT compatibility.
pub fn to_kebab_case(s: &str) -> String {
    let raw: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let mut result = String::new();
    let mut prev_hyphen = true;
    for c in raw.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    result.to_lowercase()
}

/// Expand MIR params into component params, flattening strings to (u32, u32).
fn expand_component_params(params: &[mir::function::MirParam]) -> Vec<(&'static str, ComponentValType)> {
    let mut result = Vec::new();
    let mut idx = 0;
    for p in params {
        match &p.ty {
            MirType::String(_) => {
                let name_ptr: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((name_ptr, ComponentValType::Primitive(PrimitiveValType::U32)));
                idx += 1;
                let name_len: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((name_len, ComponentValType::Primitive(PrimitiveValType::U32)));
                idx += 1;
            }
            ty => {
                let name: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((name, ComponentValType::Primitive(mir_to_prim(ty))));
                idx += 1;
            }
        }
    }
    result
}

/// Build a WASM Component from a core module + MIR metadata.
pub fn wrap_in_component(
    db: &dyn WorkspaceDataBase,
    core_wasm: &[u8],
    module: &MirModule,
) -> Result<Vec<u8>, String> {
    let mut builder = ComponentBuilder::default();

    // === Step 1: Define component types for each import ===
    let mut import_type_indices = Vec::new();
    for ext in &module.extern_functions {
        let type_idx = {
            let params = expand_component_params(&ext.params);
            let result = ext
                .return_type
                .as_ref()
                .filter(|t| !matches!(t, MirType::Void))
                .map(|t| ComponentValType::Primitive(mir_to_prim(t)));

            let (idx, mut enc) = builder.type_function(None);
            enc.params(params);
            enc.result(result);
            idx
        };
        import_type_indices.push(type_idx);
    }

    // === Step 2: Import component functions ===
    let mut comp_import_func_indices = Vec::new();
    for (i, ext) in module.extern_functions.iter().enumerate() {
        let import_name = to_kebab_case(&format!("{}-{}", ext.module, ext.import_name));
        let func_idx = builder.import(
            &import_name,
            ComponentTypeRef::Func(import_type_indices[i]),
        );
        comp_import_func_indices.push(func_idx);
    }

    // === Step 3: Canon lower each imported component function ===
    let mut lowered_core_func_indices = Vec::new();
    for (i, _ext) in module.extern_functions.iter().enumerate() {
        // For now, no strings in imports → no Memory/Realloc needed
        let core_func_idx =
            builder.lower_func(None, comp_import_func_indices[i], vec![]);
        lowered_core_func_indices.push(core_func_idx);
    }

    // === Step 4: Bundle lowered imports into a core instance ===
    // Group by original module name
    let mut module_groups: Vec<(String, Vec<(String, u32)>)> = Vec::new();
    for (i, ext) in module.extern_functions.iter().enumerate() {
        let module_name = ext.module.to_string();
        let func_name = ext.import_name.to_string();
        if let Some(group) = module_groups.iter_mut().find(|(m, _)| m == &module_name) {
            group.1.push((func_name, lowered_core_func_indices[i]));
        } else {
            module_groups.push((
                module_name,
                vec![(func_name, lowered_core_func_indices[i])],
            ));
        }
    }

    let mut import_instance_indices = Vec::new();
    let mut import_instance_names = Vec::new();
    for (module_name, funcs) in &module_groups {
        let exports: Vec<(&str, ExportKind, u32)> = funcs
            .iter()
            .map(|(name, idx)| (name.as_str(), ExportKind::Func, *idx))
            .collect();
        let inst_idx = builder.core_instantiate_exports(None, exports);
        import_instance_indices.push(inst_idx);
        import_instance_names.push(module_name.clone());
    }

    // === Step 5: Embed core module ===
    let core_module_idx = builder.core_module_raw(None, core_wasm);

    // === Step 6: Instantiate core module with import instances ===
    let instantiate_args: Vec<(&str, ModuleArg)> = import_instance_names
        .iter()
        .zip(import_instance_indices.iter())
        .map(|(name, idx)| (name.as_str(), ModuleArg::Instance(*idx)))
        .collect();
    let core_instance_idx =
        builder.core_instantiate(None, core_module_idx, instantiate_args);

    // === Step 7: Alias memory from core instance (needed for string lift/lower) ===
    let _core_memory_idx = builder.core_alias_export(
        None,
        core_instance_idx,
        "memory",
        ExportKind::Memory,
    );

    // === Step 8: Define export types + canon lift + export ===
    // Only lift test functions to the component level.
    // All other functions stay internal to the core module.
    for func in &module.functions {
        if !func.is_test {
            continue;
        }

        let raw_name = func
            .export_name
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| func.name.text(db).to_string());

        let export_name = to_kebab_case(&raw_name);

        // Define type
        let params = expand_component_params(&func.params);
        let result = func
            .return_type
            .as_ref()
            .filter(|t| !matches!(t, MirType::Void))
            .map(|t| ComponentValType::Primitive(mir_to_prim(t)));

        let (type_idx, mut enc) = builder.type_function(None);
        enc.params(params);
        enc.result(result);

        // Alias the core function from the instance
        let core_func_idx = builder.core_alias_export(
            None,
            core_instance_idx,
            &raw_name,
            ExportKind::Func,
        );

        // Lift to component function — no Memory needed since strings are u32 pairs
        let comp_func_idx =
            builder.lift_func(None, core_func_idx, type_idx, vec![]);

        // Export
        builder.export(&export_name, ComponentExportKind::Func, comp_func_idx, None);
    }

    Ok(builder.finish())
}
