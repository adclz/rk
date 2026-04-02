//! Wraps a core WASM module into a WASM Component using wasm-encoder's ComponentBuilder.
//!
//! WASI imports (module starts with "wasi:") are imported as proper interface instances.
//! Non-WASI imports are imported as flat component functions.

use db::WorkspaceDataBase;
use mir::MirModule;
use mir::types::{MirElementary, MirType};
use wasm_encoder::{
    ComponentBuilder, ComponentExportKind, ComponentTypeRef, ComponentValType, ExportKind,
    InstanceType, ModuleArg, PrimitiveValType,
};

fn mir_to_prim(ty: &MirType) -> PrimitiveValType {
    match ty {
        MirType::Elementary(e) => match e {
            MirElementary::Bool => PrimitiveValType::Bool,
            MirElementary::SInt | MirElementary::Int | MirElementary::DInt => PrimitiveValType::S32,
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
            MirElementary::Time
            | MirElementary::LTime
            | MirElementary::Date
            | MirElementary::LDate
            | MirElementary::Tod
            | MirElementary::LTod
            | MirElementary::DateAndTime
            | MirElementary::LDateTime => PrimitiveValType::U64,
            MirElementary::Char | MirElementary::WChar => PrimitiveValType::Char,
        },
        MirType::Pointer(_) | MirType::String(_) => PrimitiveValType::U32,
        _ => PrimitiveValType::S32,
    }
}

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

fn expand_component_params(
    params: &[mir::function::MirParam],
) -> Vec<(&'static str, ComponentValType)> {
    let mut result = Vec::new();
    let mut idx = 0;
    for p in params {
        match &p.ty {
            MirType::String(_) => {
                let n1: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((n1, ComponentValType::Primitive(PrimitiveValType::U32)));
                idx += 1;
                let n2: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((n2, ComponentValType::Primitive(PrimitiveValType::U32)));
                idx += 1;
            }
            ty => {
                let n: &'static str = Box::leak(format!("p{}", idx).into_boxed_str());
                result.push((n, ComponentValType::Primitive(mir_to_prim(ty))));
                idx += 1;
            }
        }
    }
    result
}

/// A WASI interface import: module path + list of functions.
struct WasiInterface {
    /// Full interface path, e.g. "wasi:clocks/monotonic-clock"
    module: String,
    /// Functions in this interface: (name, params, return_type)
    functions: Vec<WasiFunc>,
}

struct WasiFunc {
    name: String,
    params: Vec<mir::function::MirParam>,
    return_type: Option<MirType>,
    /// Index into the extern_functions list (for lowered func mapping)
    extern_idx: usize,
}

pub fn wrap_in_component(
    db: &dyn WorkspaceDataBase,
    core_wasm: &[u8],
    module: &MirModule,
) -> Result<Vec<u8>, String> {
    let mut builder = ComponentBuilder::default();

    // Separate WASI interface imports from flat imports
    let mut wasi_interfaces: Vec<WasiInterface> = Vec::new();
    let mut flat_imports: Vec<(usize, &mir::function::MirExternFunction)> = Vec::new();

    for (i, ext) in module.extern_functions.iter().enumerate() {
        if ext.module.starts_with("wasi:") {
            let iface = wasi_interfaces
                .iter_mut()
                .find(|w| w.module == ext.module.as_str());
            let func = WasiFunc {
                name: ext.import_name.to_string(),
                params: ext.params.clone(),
                return_type: ext.return_type.clone(),
                extern_idx: i,
            };
            if let Some(iface) = iface {
                iface.functions.push(func);
            } else {
                wasi_interfaces.push(WasiInterface {
                    module: ext.module.to_string(),
                    functions: vec![func],
                });
            }
        } else {
            flat_imports.push((i, ext));
        }
    }

    // === Step 1a: Import WASI interfaces as instance imports ===
    // For each WASI interface, define an instance type with its functions,
    // import the instance, alias each function, and lower it.
    let mut lowered_core_func_indices: Vec<(usize, u32)> = Vec::new(); // (extern_idx, core_func_idx)

    for iface in &wasi_interfaces {
        // Define the instance type
        let mut inst_type = InstanceType::new();
        for (func_i, func) in iface.functions.iter().enumerate() {
            let params = expand_component_params(&func.params);
            let result = func
                .return_type
                .as_ref()
                .filter(|t| !matches!(t, MirType::Void))
                .map(|t| ComponentValType::Primitive(mir_to_prim(t)));
            let mut enc = inst_type.ty().function();
            enc.params(params);
            enc.result(result);
            inst_type.export(&func.name, ComponentTypeRef::Func(func_i as u32));
        }

        let inst_type_idx = builder.type_instance(None, &inst_type);

        // Import the instance
        let instance_idx = builder.import(&iface.module, ComponentTypeRef::Instance(inst_type_idx));

        // Alias each function from the instance and lower it
        for func in &iface.functions {
            let comp_func_idx =
                builder.alias_export(instance_idx, &func.name, ComponentExportKind::Func);
            let core_func_idx = builder.lower_func(None, comp_func_idx, vec![]);
            lowered_core_func_indices.push((func.extern_idx, core_func_idx));
        }
    }

    // === Step 1b: Define types + import flat (non-WASI) functions ===
    for (ext_idx, ext) in &flat_imports {
        let params = expand_component_params(&ext.params);
        let result = ext
            .return_type
            .as_ref()
            .filter(|t| !matches!(t, MirType::Void))
            .map(|t| ComponentValType::Primitive(mir_to_prim(t)));
        let (type_idx, mut enc) = builder.type_function(None);
        enc.params(params);
        enc.result(result);

        let import_name = to_kebab_case(&format!("{}-{}", ext.module, ext.import_name));
        let comp_func_idx = builder.import(&import_name, ComponentTypeRef::Func(type_idx));
        let core_func_idx = builder.lower_func(None, comp_func_idx, vec![]);
        lowered_core_func_indices.push((*ext_idx, core_func_idx));
    }

    // === Step 2: Bundle lowered imports into core instances (grouped by module) ===
    // Sort lowered funcs by their extern_idx to maintain order
    lowered_core_func_indices.sort_by_key(|(idx, _)| *idx);

    let mut module_groups: Vec<(String, Vec<(String, u32)>)> = Vec::new();
    for (ext_idx, core_func_idx) in &lowered_core_func_indices {
        let ext = &module.extern_functions[*ext_idx];
        let module_name = ext.module.to_string();
        let func_name = ext.import_name.to_string();
        if let Some(group) = module_groups.iter_mut().find(|(m, _)| m == &module_name) {
            group.1.push((func_name, *core_func_idx));
        } else {
            module_groups.push((module_name, vec![(func_name, *core_func_idx)]));
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

    // === Step 3: Embed + instantiate core module ===
    let core_module_idx = builder.core_module_raw(None, core_wasm);
    let instantiate_args: Vec<(&str, ModuleArg)> = import_instance_names
        .iter()
        .zip(import_instance_indices.iter())
        .map(|(name, idx)| (name.as_str(), ModuleArg::Instance(*idx)))
        .collect();
    let core_instance_idx = builder.core_instantiate(None, core_module_idx, instantiate_args);

    // Alias memory
    let _core_memory_idx =
        builder.core_alias_export(None, core_instance_idx, "memory", ExportKind::Memory);

    // === Step 4: Lift + export test functions ===
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

        let params = expand_component_params(&func.params);
        let result = func
            .return_type
            .as_ref()
            .filter(|t| !matches!(t, MirType::Void))
            .map(|t| ComponentValType::Primitive(mir_to_prim(t)));
        let (type_idx, mut enc) = builder.type_function(None);
        enc.params(params);
        enc.result(result);

        let core_func_idx =
            builder.core_alias_export(None, core_instance_idx, &raw_name, ExportKind::Func);
        let comp_func_idx = builder.lift_func(None, core_func_idx, type_idx, vec![]);
        builder.export(&export_name, ComponentExportKind::Func, comp_func_idx, None);
    }

    Ok(builder.finish())
}
