// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Classic source-level stack traces on wasmtime 45 — no guest_debug, no RR.
//! When a compiled PLC traps, wasmtime's `WasmBacktrace` gives the frame chain
//! (`func_index` + `module_offset`), and we resolve each frame to its IEC
//! function name (`debug-functions`) and source line (`debug-lines`).

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use debug_format::DebugInfo;
use rstest::*;

/// 0-based source line of the first occurrence of `needle`.
fn row_of(src: &str, needle: &str) -> u32 {
    let byte = src
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` not in source"));
    src[..byte].bytes().filter(|&b| b == b'\n').count() as u32
}

/// A conversion out of range deep in a call chain traps; the backtrace
/// resolves to a source-level stack: the right IEC function names and the
/// right source lines.
#[rstest]
fn trap_yields_source_level_stack_trace(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION boom : DINT
        VAR_INPUT n : REAL; END_VAR
            {wasm boom 'trunc_f32_s' (params n) (result boom)}
        END_FUNCTION

        FUNCTION caller : DINT
        VAR_INPUT m : REAL; END_VAR
            caller := boom(n := m);
        END_FUNCTION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let dbg = DebugInfo::from_wasm(&wasm);

    // Run `caller(1e30)` → boom(1e30) → past DINT → the conversion traps.
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).expect("module");
    // `defined_index = func_index - imported funcs` (a frame's func_index is
    // module-level, including imports).
    let n_imports = module
        .imports()
        .filter(|i| matches!(i.ty(), wasmtime::ExternType::Func(_)))
        .count() as u32;
    let mut store = wasmtime::Store::new(&engine, ());
    let linker = wasmtime::Linker::new(&engine);
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("instantiate");
    let caller = instance
        .get_typed_func::<f32, i32>(&mut store, "caller")
        .expect("caller export");
    let err = caller
        .call(&mut store, 1.0e30)
        .expect_err("a conversion out of range must trap");

    // The trap carries a WasmBacktrace; resolve every frame to IEC name + line.
    let bt = err
        .downcast_ref::<wasmtime::WasmBacktrace>()
        .expect("trap carries a WasmBacktrace");

    // Convert the raw backtrace to our source-level stack trace in one call.
    let frames = crate::tests::codegen::resolve_backtrace(&dbg, bt, n_imports);
    eprintln!("source-level stack trace: {frames:#?}");

    // Both IEC functions are named, innermost first, each at its source line.
    let line_of = |needle: &str| {
        frames
            .iter()
            .find(|f| f.function.as_deref().is_some_and(|n| n.contains(needle)))
            .unwrap_or_else(|| panic!("`{needle}` frame missing in {frames:?}"))
            .source
            .as_ref()
            .map(|s| s.line)
    };
    assert_eq!(
        line_of("boom"),
        Some(row_of(source, "{wasm boom 'trunc_f32_s'")),
        "boom frame points at the trapping conversion"
    );
    assert_eq!(
        line_of("caller"),
        Some(row_of(source, "boom(n := m)")),
        "caller frame points at the call site"
    );
}
