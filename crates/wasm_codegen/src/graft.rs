//! Graft math intrinsics from `wasm_builtins.wasm` into the output module:
//! the union of the requested builtins' transitive closures, a fresh type
//! index per signature and function index per body, every
//! `Call $bundle_idx` rewritten to `Call $output_idx`, bodies appended to
//! the code section.

use rustc_hash::{FxHashMap, FxHashSet};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, Function, FunctionSection, GlobalSection, GlobalType,
    Instruction, TypeSection,
};

use crate::builtins::{
    BUILTIN_DATA, BUILTIN_FUNCS, BUILTIN_GLOBALS, BUILTIN_IMPORTS, BUILTIN_NAMES, BUILTIN_SIGS,
    BUNDLE_MEMORY_TOP, BuiltinCallSite, BuiltinFunc, BuiltinSig, N_IMPORTS, closure_uses_import,
    transitive_closure,
};

/// Wasm import index of `__iec_raise`, computed rather than assumed.
fn rk_raise_import_wasm_idx() -> Option<u32> {
    BUILTIN_IMPORTS
        .iter()
        .position(|imp| imp.name == "__iec_raise")
        .map(|p| p as u32)
}

/// Result of grafting all referenced builtins.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct GraftPlan {
    /// Number of builtin functions appended (after imports, before user fns).
    /// Includes the synthesized `__iec_raise` helper when one is emitted.
    pub n_funcs: u32,
    /// Number of new types appended for builtin signatures.
    pub n_types: u32,
    /// Builtin name (e.g. `f32.sin`) → output WASM function index.
    pub name_to_wasm_idx: FxHashMap<String, u32>,
    /// Lowest memory byte the IEC layout may use without colliding with the
    /// bundle's data segments. `0` if no graft happened.
    pub rk_layout_floor: u32,
}

/// Append every builtin in `names_used` and its helpers to the caller's
/// sections, which must place them after imports and before user
/// functions. The bundle's stack-pointer global and data go in on the
/// first graft.
#[allow(clippy::too_many_arguments)]
pub(crate) fn graft_builtins<'a>(
    names_used: impl IntoIterator<Item = &'a str>,
    type_section: &mut TypeSection,
    fn_section: &mut FunctionSection,
    code_section: &mut CodeSection,
    global_section: &mut GlobalSection,
    data_section: &mut DataSection,
    next_type_idx: &mut u32,
    base_fn_idx: u32,
    rk_exception_tag_idx: Option<u32>,
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

    // Allocate output indices. Layout:
    //   [synth __iec_raise helper (if any grafted body calls it)]
    //   [grafted bundle bodies, visit-order]
    let needs_iec_raise = closure_uses_import(&visit_order, "__iec_raise");
    if needs_iec_raise && rk_exception_tag_idx.is_none() {
        panic!(
            "grafted builtins call `__iec_raise` but the caller did not register the \
             `$rk_exception` tag — register it before invoking graft_builtins and pass \
             its index in `rk_exception_tag_idx`"
        );
    }

    let mut bundle_to_wasm: FxHashMap<u32, u32> = FxHashMap::default();
    let mut sig_to_type_idx: FxHashMap<u32, u32> = FxHashMap::default();
    let mut import_to_wasm: FxHashMap<u32, u32> = FxHashMap::default();
    let mut next_fn_idx = base_fn_idx;
    let mut n_types = 0u32;

    // 1) The synth helper, first so it sits at `base_fn_idx`:
    //    `(i32, i32) -> ()`, throws the IEC exception tag and ends with
    //    `unreachable`.
    if needs_iec_raise {
        let import_wasm_idx = rk_raise_import_wasm_idx()
            .expect("__iec_raise import must exist when needs_iec_raise is true");
        let import_sig = BUILTIN_IMPORTS[import_wasm_idx as usize].sig_idx;

        let wasm_type_idx = *sig_to_type_idx.entry(import_sig).or_insert_with(|| {
            let sig: &BuiltinSig = &BUILTIN_SIGS[import_sig as usize];
            type_section
                .ty()
                .function(sig.params.iter().copied(), sig.results.iter().copied());
            let idx = *next_type_idx;
            *next_type_idx += 1;
            n_types += 1;
            idx
        });

        fn_section.function(wasm_type_idx);

        // Body: local.get 0; local.get 1; throw $rk_exception; unreachable
        let mut body = Function::new(Vec::new());
        body.instruction(&Instruction::LocalGet(0));
        body.instruction(&Instruction::LocalGet(1));
        body.instruction(&Instruction::Throw(rk_exception_tag_idx.unwrap()));
        body.instruction(&Instruction::Unreachable);
        body.instruction(&Instruction::End);
        code_section.function(&body);

        import_to_wasm.insert(import_wasm_idx, next_fn_idx);
        next_fn_idx += 1;
    }

    // 2) Allocate output indices for grafted bodies in visit order.
    for &bundle_idx in &visit_order {
        let entry: &BuiltinFunc = &BUILTIN_FUNCS[bundle_idx as usize];

        // Register the function's signature in the output type section
        // (deduped across builtins that share a signature).
        let wasm_type_idx = *sig_to_type_idx.entry(entry.sig_idx).or_insert_with(|| {
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

    // 3) Rewrite bodies and append, in the same visit order as the
    //    function section.
    for &bundle_idx in &visit_order {
        let entry = &BUILTIN_FUNCS[bundle_idx as usize];
        let new_body = rewrite_body(entry, &bundle_to_wasm, &import_to_wasm);
        code_section.raw(&new_body);
    }

    // Build the public name → wasm idx map for the call-emit phase.
    let mut name_to_wasm_idx = FxHashMap::default();
    for (name, root) in name_roots {
        if let Some(&wasm_idx) = bundle_to_wasm.get(&root) {
            name_to_wasm_idx.insert(name, wasm_idx);
        }
    }

    let any_grafted = !visit_order.is_empty() || needs_iec_raise;
    if any_grafted && !visit_order.is_empty() {
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

    let n_funcs = visit_order.len() as u32 + if needs_iec_raise { 1 } else { 0 };
    GraftPlan {
        n_funcs,
        n_types,
        name_to_wasm_idx,
        rk_layout_floor: if any_grafted { BUNDLE_MEMORY_TOP } else { 0 },
    }
}

/// Patch every `Call $old_idx` in `entry.body` to `Call $new_idx`: an
/// import (`< N_IMPORTS`) via `import_to_wasm`, a defined function via
/// `bundle_to_wasm`. The LEB128 operand is re-encoded.
fn rewrite_body(
    entry: &BuiltinFunc,
    bundle_to_wasm: &FxHashMap<u32, u32>,
    import_to_wasm: &FxHashMap<u32, u32>,
) -> Vec<u8> {
    let mut new = Vec::with_capacity(entry.body.len());
    let mut cursor = 0usize;
    for cs in entry.call_sites {
        let cs: &BuiltinCallSite = cs;
        let op_off = cs.operand_offset as usize;
        // Copy verbatim up to and including the Call opcode (just before
        // the operand).
        new.extend_from_slice(&entry.body[cursor..op_off]);
        // Encode the new target as ULEB128.
        let new_target = if cs.target < N_IMPORTS {
            import_to_wasm.get(&cs.target).copied().expect(
                "call to import has no synth helper — graft must register one before \
                 rewriting bodies that reference it",
            )
        } else {
            let bundle_idx = cs.target - N_IMPORTS;
            bundle_to_wasm
                .get(&bundle_idx)
                .copied()
                .expect("call target not in graft set - closure walk missed a callee")
        };
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
            /* rk_exception_tag_idx = */ None,
        );
        let sin_idx = plan.name_to_wasm_idx["f32.sin"];
        assert!(plan.rk_layout_floor > 0);

        types
            .ty()
            .function([wasm_encoder::ValType::F32], [wasm_encoder::ValType::F32]);
        let wrapper_type = next_type;
        funcs.function(wrapper_type);

        let mut wrapper = wasm_encoder::Function::new(Vec::new());
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(0));
        wrapper.instruction(&wasm_encoder::Instruction::Call(sin_idx));
        wrapper.instruction(&wasm_encoder::Instruction::End);
        code.function(&wrapper);

        let mut exports = wasm_encoder::ExportSection::new();
        exports.export("wrapper", wasm_encoder::ExportKind::Func, plan.n_funcs);

        // 1 page of memory is enough - bundle was built with -zstack-size=8192
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
            None,
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
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
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
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
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

    /// Graft `rk.div_i32_checked` — a Rust builtin that does a plain
    /// `a / b`. Rust's compiler emits a pre-check + `panic!()` for
    /// divide-by-zero and `INT_MIN / -1`; the bundle's panic handler
    /// re-throws that panic as `$rk_exception`. End-to-end test of
    /// the "Rust panic → IEC exception" path: build.rs picked up the
    /// `__iec_raise` import, graft synthesized a helper, Rust's panic
    /// constant string landed in the bundle's `.rodata`-style data
    /// segment, and wasm exception delivery hands the (ptr, len) back
    /// to a `catch` clause that surfaces the bytes verbatim.
    #[test]
    fn checked_div_raises_division_by_zero() {
        let mut types = TypeSection::new();
        let mut funcs = FunctionSection::new();
        let mut code = CodeSection::new();
        let mut globals = GlobalSection::new();
        let mut data = DataSection::new();
        let mut next_type = 0u32;

        // Register `$rk_exception` tag's func type — (i32, i32) -> ()
        types.ty().function(
            [wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
            std::iter::empty::<wasm_encoder::ValType>(),
        );
        let tag_type_idx = next_type;
        next_type += 1;
        let tag_idx = 0u32;

        let plan = graft_builtins(
            ["rk.div_i32_checked"],
            &mut types,
            &mut funcs,
            &mut code,
            &mut globals,
            &mut data,
            &mut next_type,
            /* base_fn_idx = */ 0,
            Some(tag_idx),
        );
        let div_idx = plan.name_to_wasm_idx["rk.div_i32_checked"];

        // Wrapper: `(numerator, divisor) -> (thrown_flag, msg_ptr, msg_len)`.
        // Catches `$rk_exception` via try_table; on the success path
        // discards the integer division result and returns `(0, 0, 0)`,
        // on the catch path returns `(1, ptr, len)` where `(ptr, len)`
        // is the exception payload.
        types.ty().function(
            [wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
            [
                wasm_encoder::ValType::I32,
                wasm_encoder::ValType::I32,
                wasm_encoder::ValType::I32,
            ],
        );
        let wrapper_type = next_type;
        next_type += 1;
        funcs.function(wrapper_type);

        // Block type `() -> (i32 i32)` — the catch hands (ptr, len) to
        // $on_catch as its result.
        types.ty().function(
            std::iter::empty::<wasm_encoder::ValType>(),
            [wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
        );
        let on_catch_ty = next_type;
        next_type += 1;

        // Locals 2,3 stash (ptr, len) from the catch landing pad before
        // we re-push them after the thrown_flag.
        let mut wrapper = wasm_encoder::Function::new([(2, wasm_encoder::ValType::I32)]);
        wrapper.instruction(&wasm_encoder::Instruction::Block(
            wasm_encoder::BlockType::FunctionType(on_catch_ty),
        ));
        wrapper.instruction(&wasm_encoder::Instruction::TryTable(
            wasm_encoder::BlockType::Empty,
            std::borrow::Cow::Owned(vec![wasm_encoder::Catch::One {
                tag: tag_idx,
                label: 0, // → $on_catch (outer scope of try_table)
            }]),
        ));
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(0));
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(1));
        wrapper.instruction(&wasm_encoder::Instruction::Call(div_idx));
        wrapper.instruction(&wasm_encoder::Instruction::Drop);
        wrapper.instruction(&wasm_encoder::Instruction::End); // try_table
        // Success: return (0, 0, 0)
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(0));
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(0));
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(0));
        wrapper.instruction(&wasm_encoder::Instruction::Return);
        wrapper.instruction(&wasm_encoder::Instruction::End); // $on_catch
        // Catch landing: (ptr, len) on stack — stash, then push (1, ptr, len).
        wrapper.instruction(&wasm_encoder::Instruction::LocalSet(3)); // len
        wrapper.instruction(&wasm_encoder::Instruction::LocalSet(2)); // ptr
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(1));
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(2));
        wrapper.instruction(&wasm_encoder::Instruction::LocalGet(3));
        wrapper.instruction(&wasm_encoder::Instruction::End); // func
        code.function(&wrapper);

        let wrapper_idx = plan.n_funcs;

        let mut exports = wasm_encoder::ExportSection::new();
        exports.export("try_div", wasm_encoder::ExportKind::Func, wrapper_idx);
        exports.export("memory", wasm_encoder::ExportKind::Memory, 0);

        let mut memories = wasm_encoder::MemorySection::new();
        memories.memory(wasm_encoder::MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut tags = wasm_encoder::TagSection::new();
        tags.tag(wasm_encoder::TagType {
            kind: wasm_encoder::TagKind::Exception,
            func_type_idx: tag_type_idx,
        });

        let mut module = wasm_encoder::Module::new();
        module.section(&types);
        module.section(&funcs);
        module.section(&memories);
        module.section(&tags);
        module.section(&globals);
        module.section(&exports);
        module.section(&code);
        module.section(&data);
        let bytes = module.finish();
        let _ = next_type;

        wasmparser::Validator::new_with_features({
            let mut f = wasmparser::WasmFeatures::default();
            f.insert(wasmparser::WasmFeatures::EXCEPTIONS);
            f
        })
        .validate_all(&bytes)
        .expect("module with exceptions must validate");

        let mut config = wasmtime::Config::new();
        config.wasm_exceptions(true);
        let engine = Engine::new(&config).unwrap();
        let module = Module::new(&engine, &bytes).expect("compile");
        let mut store = Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let try_div = instance
            .get_typed_func::<(i32, i32), (i32, i32, i32)>(&mut store, "try_div")
            .unwrap();
        let memory = instance.get_memory(&mut store, "memory").unwrap();

        // Happy path: 10 / 2 = 5. No throw, thrown_flag must be 0.
        let (thrown, _ptr, _len) = try_div.call(&mut store, (10, 2)).unwrap();
        assert_eq!(thrown, 0, "division by non-zero must not throw");

        // Faulting path: 10 / 0 → Rust panics, panic handler throws
        // `$rk_exception` with Rust's verbatim panic-message text.
        let (thrown, ptr, len) = try_div.call(&mut store, (10, 0)).unwrap();
        assert_eq!(thrown, 1, "division by zero must throw");
        assert!(len > 0, "expected a non-empty payload");
        let mut buf = vec![0u8; len as usize];
        memory.read(&store, ptr as usize, &mut buf).unwrap();
        let msg = core::str::from_utf8(&buf).expect("payload must be utf-8");
        assert_eq!(
            msg, "attempt to divide by zero",
            "exception payload must match the compiler-emitted panic message"
        );

        // Overflow edge case: i32::MIN / -1 — same panic-handler path.
        let (thrown, ptr, len) = try_div.call(&mut store, (i32::MIN, -1)).unwrap();
        assert_eq!(thrown, 1, "INT_MIN / -1 must throw");
        let mut buf = vec![0u8; len as usize];
        memory.read(&store, ptr as usize, &mut buf).unwrap();
        let msg = core::str::from_utf8(&buf).expect("payload must be utf-8");
        assert_eq!(msg, "attempt to divide with overflow");
    }

    /// Graft `rk.raise_str` (the trampoline that calls the `__iec_raise`
    /// import from `wasm_builtins`) into a tiny output module and verify
    /// that calling it throws the codegen-registered `$rk_exception`
    /// tag. This exercises the full pipeline:
    ///   1. build.rs parsed the import out of wasm_builtins.wasm
    ///   2. transitive_closure reached an import call site
    ///   3. graft_builtins synthesized an `__iec_raise` helper
    ///   4. rewrite_body redirected the Call from the import idx to the
    ///      synth helper idx
    ///   5. running the wrapper throws and wasmtime surfaces it
    #[test]
    fn rk_raise_str_throws_iec_exception() {
        let mut types = TypeSection::new();
        let mut funcs = FunctionSection::new();
        let mut code = CodeSection::new();
        let mut globals = GlobalSection::new();
        let mut data = DataSection::new();
        let mut next_type = 0u32;

        // Pre-register the `$rk_exception` tag's type. Graft expects the
        // tag idx to be known. Tag itself goes into a TagSection emitted
        // below.
        types.ty().function(
            [wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
            std::iter::empty::<wasm_encoder::ValType>(),
        );
        let tag_type_idx = next_type;
        next_type += 1;
        let tag_idx = 0u32;

        let plan = graft_builtins(
            ["rk.raise_str"],
            &mut types,
            &mut funcs,
            &mut code,
            &mut globals,
            &mut data,
            &mut next_type,
            /* base_fn_idx = */ 0,
            Some(tag_idx),
        );
        let target_idx = plan.name_to_wasm_idx["rk.raise_str"];

        // Wrapper: calls rk_raise_str with a fixed (ptr, len). The bytes
        // pointed at don't matter for the throw path — wasmtime just
        // surfaces an "uncaught exception" trap. We only check that the
        // call traps and that the resulting wasm validates.
        types.ty().function(
            std::iter::empty::<wasm_encoder::ValType>(),
            std::iter::empty::<wasm_encoder::ValType>(),
        );
        let wrapper_type = next_type;
        next_type += 1;
        funcs.function(wrapper_type);

        let mut wrapper = wasm_encoder::Function::new(Vec::new());
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(0));
        wrapper.instruction(&wasm_encoder::Instruction::I32Const(0));
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

        // Emit a TagSection referencing the pre-registered type so the
        // `throw` in the synth helper has a valid tag to reference.
        let mut tags = wasm_encoder::TagSection::new();
        tags.tag(wasm_encoder::TagType {
            kind: wasm_encoder::TagKind::Exception,
            func_type_idx: tag_type_idx,
        });

        let mut module = wasm_encoder::Module::new();
        module.section(&types);
        module.section(&funcs);
        module.section(&memories);
        module.section(&tags);
        module.section(&globals);
        module.section(&exports);
        module.section(&code);
        module.section(&data);
        let bytes = module.finish();
        let _ = next_type;

        wasmparser::Validator::new_with_features({
            let mut f = wasmparser::WasmFeatures::default();
            f.insert(wasmparser::WasmFeatures::EXCEPTIONS);
            f
        })
        .validate_all(&bytes)
        .expect("module with exceptions must validate");

        let mut config = wasmtime::Config::new();
        config.wasm_exceptions(true);
        let engine = Engine::new(&config).unwrap();
        let module = Module::new(&engine, &bytes).expect("compile");
        let mut store = Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiate");
        let f = instance
            .get_typed_func::<(), ()>(&mut store, "wrapper")
            .unwrap();
        let err = f.call(&mut store, ()).expect_err("must trap on throw");
        // Uncaught at the wasm level, wasmtime reports `Trap::UncaughtException`.
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("exception") || msg.to_lowercase().contains("trap"),
            "expected exception-trap error, got: {msg}"
        );
    }
}
