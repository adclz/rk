//! Graft math intrinsics from `wasm_builtins.wasm` into the output module:
//! the union of the requested builtins' transitive closures, a fresh type
//! index per signature and function index per body, every
//! `Call $bundle_idx` rewritten to `Call $output_idx`, bodies appended to
//! the code section.

use rustc_hash::{FxHashMap, FxHashSet};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, FunctionSection, GlobalSection, GlobalType, TypeSection,
};

use crate::builtins::{
    BUILTIN_DATA, BUILTIN_FUNCS, BUILTIN_GLOBALS, BUILTIN_NAMES, BUILTIN_SIGS, BUNDLE_MEMORY_TOP,
    BuiltinCallSite, BuiltinFunc, BuiltinSig, transitive_closure,
};

/// Result of grafting all referenced builtins.
pub(crate) struct GraftPlan {
    /// Number of builtin functions appended (after imports, before user fns).
    pub n_funcs: u32,
    /// Number of new types appended for builtin signatures.
    pub n_types: u32,
    /// Builtin name (e.g. `f32.sin`) → output WASM function index.
    pub name_to_wasm_idx: FxHashMap<String, u32>,
    /// Lowest memory byte the IEC layout may use without colliding with the
    /// bundle's data segments. `0` if no graft happened.
    pub rk_layout_floor: u32,
}

/// Append every builtin in `names_used` (and its transitive helpers) to the
/// caller's sections. The caller is responsible for placing the function
/// emissions *after* imports and *before* user-defined functions so the
/// resulting indices line up.
///
/// `global_section` and `data_section` receive the bundle's stack-pointer
/// global and polynomial-coefficient data on the first call to graft —
/// these are fixed-format and shared across every grafted body. If no
/// builtins are referenced, both sections are left untouched.
pub(crate) fn graft_builtins<'a>(
    names_used: impl IntoIterator<Item = &'a str>,
    type_section: &mut TypeSection,
    fn_section: &mut FunctionSection,
    code_section: &mut CodeSection,
    global_section: &mut GlobalSection,
    data_section: &mut DataSection,
    next_type_idx: &mut u32,
    base_fn_idx: u32,
) -> GraftPlan {
    // Collect every bundle function that needs to ship in this output.
    let mut visit_order: Vec<u32> = Vec::new();
    let mut seen = FxHashSet::default();
    let mut name_roots: Vec<(String, u32)> = Vec::new();
    for name in names_used {
        let Some(root) = BUILTIN_NAMES.get(name).copied() else {
            continue;
        };
        name_roots.push((name.to_string(), root));
        for idx in transitive_closure(root) {
            if seen.insert(idx) {
                visit_order.push(idx);
            }
        }
    }

    // Allocate output indices in visit order.
    let mut bundle_to_wasm: FxHashMap<u32, u32> = FxHashMap::default();
    let mut sig_to_type_idx: FxHashMap<u32, u32> = FxHashMap::default();
    let mut next_fn_idx = base_fn_idx;
    let mut n_types = 0u32;

    for &bundle_idx in &visit_order {
        let entry: &BuiltinFunc = &BUILTIN_FUNCS[bundle_idx as usize];

        // Register the function's signature in the output type section
        // (deduped across builtins that share a signature).
        let wasm_type_idx = *sig_to_type_idx
            .entry(entry.sig_idx)
            .or_insert_with(|| {
                let sig: &BuiltinSig = &BUILTIN_SIGS[entry.sig_idx as usize];
                type_section
                    .ty()
                    .function(sig.params.iter().copied(), sig.results.iter().copied());
                let idx = *next_type_idx;
                *next_type_idx += 1;
                n_types += 1;
                idx
            });

        fn_section.function(wasm_type_idx);
        bundle_to_wasm.insert(bundle_idx, next_fn_idx);
        next_fn_idx += 1;
    }

    // Rewrite bodies and append to the code section. Must happen in the same
    // visit order as function-section emission so indices line up.
    for &bundle_idx in &visit_order {
        let entry = &BUILTIN_FUNCS[bundle_idx as usize];
        let new_body = rewrite_body(entry, &bundle_to_wasm);
        code_section.raw(&new_body);
    }

    // Build the public name → wasm idx map for the call-emit phase.
    let mut name_to_wasm_idx = FxHashMap::default();
    for (name, root) in name_roots {
        if let Some(&wasm_idx) = bundle_to_wasm.get(&root) {
            name_to_wasm_idx.insert(name, wasm_idx);
        }
    }

    let any_grafted = !visit_order.is_empty();
    if any_grafted {
        // The bundle's globals; the output had none, so no relocation.
        for g in BUILTIN_GLOBALS {
            global_section.global(
                GlobalType {
                    val_type: g.val_type,
                    mutable: g.mutable,
                    shared: false,
                },
                &ConstExpr::i32_const(g.init_i32),
            );
        }
        // The bundle's data segments at their offsets; both target memory 0.
        for d in BUILTIN_DATA {
            data_section.active(
                /* memory_index = */ 0,
                &ConstExpr::i32_const(d.memory_offset as i32),
                d.bytes.iter().copied(),
            );
        }
    }

    GraftPlan {
        n_funcs: visit_order.len() as u32,
        n_types,
        name_to_wasm_idx,
        rk_layout_floor: if any_grafted { BUNDLE_MEMORY_TOP } else { 0 },
    }
}

/// Patch every `Call $old_idx` in `entry.body` to `Call $new_idx`, where
/// `new_idx = bundle_to_wasm[old_idx]`. The original LEB128 operand is
/// replaced byte-for-byte; LEB widths may differ if the new index is larger.
fn rewrite_body(entry: &BuiltinFunc, bundle_to_wasm: &FxHashMap<u32, u32>) -> Vec<u8> {
    let mut new = Vec::with_capacity(entry.body.len());
    let mut cursor = 0usize;
    for cs in entry.call_sites {
        let cs: &BuiltinCallSite = cs;
        let op_off = cs.operand_offset as usize;
        // Copy verbatim up to and including the Call opcode (just before
        // the operand).
        new.extend_from_slice(&entry.body[cursor..op_off]);
        // Encode the new target as ULEB128.
        let new_target = bundle_to_wasm
            .get(&cs.target)
            .copied()
            .expect("call target not in graft set — closure walk missed a callee");
        write_uleb128(&mut new, new_target);
        // Skip past the original operand bytes.
        cursor = op_off + cs.operand_width as usize;
    }
    new.extend_from_slice(&entry.body[cursor..]);
    new
}

fn write_uleb128(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
            out.push(byte);
        } else {
            out.push(byte);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb128_roundtrip() {
        for v in [0u32, 1, 127, 128, 16_383, 16_384, 1_000_000, u32::MAX] {
            let mut buf = Vec::new();
            write_uleb128(&mut buf, v);
            // Decode back.
            let mut out = 0u64;
            let mut shift = 0;
            for &b in &buf {
                out |= ((b & 0x7f) as u64) << shift;
                shift += 7;
                if b & 0x80 == 0 {
                    break;
                }
            }
            assert_eq!(out as u32, v);
        }
    }

    #[test]
    fn graft_f32_sin_produces_runnable_module() {
        let mut types = TypeSection::new();
        let mut funcs = FunctionSection::new();
        let mut code = CodeSection::new();
        let mut globals = GlobalSection::new();
        let mut data = DataSection::new();
        let mut next_type = 0u32;

        let plan = graft_builtins(
            ["f32.sin"],
            &mut types,
            &mut funcs,
            &mut code,
            &mut globals,
            &mut data,
            &mut next_type,
            /* base_fn_idx = */ 0,
        );
        let sin_idx = plan.name_to_wasm_idx["f32.sin"];
        assert!(plan.rk_layout_floor > 0);

        types.ty().function(
            [wasm_encoder::ValType::F32],
            [wasm_encoder::ValType::F32],
        );
        let wrapper_type = next_type;
        funcs.function(wrapper_type);

        let mut wrapper = wasm_encoder::Function::new(Vec::new());
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(0));
        wrapper.instruction(&wasm_encoder::Instruction::Call(sin_idx));
        wrapper.instruction(&wasm_encoder::Instruction::End);
        code.function(&wrapper);

        let mut exports = wasm_encoder::ExportSection::new();
        exports.export(
            "wrapper",
            wasm_encoder::ExportKind::Func,
            plan.n_funcs,
        );

        // 1 page of memory is enough — bundle was built with -zstack-size=8192
        // so its data lives below 64 KiB.
        let mut memories = wasm_encoder::MemorySection::new();
        memories.memory(wasm_encoder::MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut module = wasm_encoder::Module::new();
        module.section(&types);
        module.section(&funcs);
        module.section(&memories);
        module.section(&globals);
        module.section(&exports);
        module.section(&code);
        module.section(&data);
        let bytes = module.finish();

        wasmparser::validate(&bytes).expect("grafted module must validate");
    }
}

#[cfg(test)]
mod execute_tests {
    //! Run the grafted module under wasmtime: libm's `sinf` must answer
    //! through the rewritten body.

    use super::*;
    use wasmtime::{Engine, Module, Store};

    fn build_wrapper_module(name: &str) -> (Vec<u8>, u32, u32) {
        let mut types = TypeSection::new();
        let mut funcs = FunctionSection::new();
        let mut code = CodeSection::new();
        let mut globals = GlobalSection::new();
        let mut data = DataSection::new();
        let mut next_type = 0u32;
        let plan = graft_builtins(
            [name],
            &mut types,
            &mut funcs,
            &mut code,
            &mut globals,
            &mut data,
            &mut next_type,
            0,
        );
        let target_idx = plan.name_to_wasm_idx[name];

        // Wrapper exports the builtin under a stable name. f32 vs f64
        // signature picked from the bundle.
        let is_f64 = name.starts_with("f64");
        let vt = if is_f64 {
            wasm_encoder::ValType::F64
        } else {
            wasm_encoder::ValType::F32
        };
        types.ty().function([vt], [vt]);
        let wrapper_type = next_type;
        funcs.function(wrapper_type);

        let mut wrapper = wasm_encoder::Function::new(Vec::new());
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(0));
        wrapper.instruction(&wasm_encoder::Instruction::Call(target_idx));
        wrapper.instruction(&wasm_encoder::Instruction::End);
        code.function(&wrapper);

        let wrapper_idx = plan.n_funcs;

        let mut exports = wasm_encoder::ExportSection::new();
        exports.export("wrapper", wasm_encoder::ExportKind::Func, wrapper_idx);

        let mut memories = wasm_encoder::MemorySection::new();
        memories.memory(wasm_encoder::MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut module = wasm_encoder::Module::new();
        module.section(&types);
        module.section(&funcs);
        module.section(&memories);
        module.section(&globals);
        module.section(&exports);
        module.section(&code);
        module.section(&data);
        (module.finish(), plan.n_funcs, plan.rk_layout_floor)
    }

    #[test]
    fn execute_f32_sin() {
        let (bytes, _, _) = build_wrapper_module("f32.sin");
        let engine = Engine::default();
        let module = Module::new(&engine, &bytes).expect("validate");
        let mut store = Store::new(&engine, ());
        let instance =
            wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let f = instance
            .get_typed_func::<f32, f32>(&mut store, "wrapper")
            .unwrap();
        let r = f.call(&mut store, 0.5).unwrap();
        assert!(
            (r - 0.5f32.sin()).abs() < 1e-6,
            "sin(0.5) got {r}, expected {}",
            0.5f32.sin()
        );
    }

    #[test]
    fn execute_f64_cos() {
        let (bytes, _, _) = build_wrapper_module("f64.cos");
        let engine = Engine::default();
        let module = Module::new(&engine, &bytes).expect("validate");
        let mut store = Store::new(&engine, ());
        let instance =
            wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let f = instance
            .get_typed_func::<f64, f64>(&mut store, "wrapper")
            .unwrap();
        let r = f.call(&mut store, 1.0).unwrap();
        assert!(
            (r - 1.0f64.cos()).abs() < 1e-12,
            "cos(1.0) got {r}, expected {}",
            1.0f64.cos()
        );
    }
}
