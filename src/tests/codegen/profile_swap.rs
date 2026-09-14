//! The invariant the debug-attach swap rests on: optimizing a module changes
//! its CODE but not its MEMORY.
//!
//! A production runtime runs the plain (optionally `-O`) build. Attaching a
//! stepping debugger means re-instantiating the *same source* in the debug
//! profile and carrying live state across. That is only sound if the two
//! builds agree on where everything lives — the retain layout, the monitoring
//! symbol table, the globals band, the schedule. All of those are decided in
//! MIR, before wasm-opt runs, so wasm-opt (which rewrites code offsets) must
//! leave them byte-identical.
//!
//! These tests pin that. If a future change makes a memory-describing section
//! depend on code layout, the swap would silently restore state into the wrong
//! addresses — so this is a guard against a corruption bug, not a nicety.
//!
//! The one section that legitimately goes stale under optimization is
//! `debug-lines` (source line -> code PC): its bytes survive but the PCs it
//! names have moved. That is why line-stepping needs the unoptimized build,
//! and it is asserted here as the *documented* exception rather than left
//! implicit.

use db::RootDatabase;
use rstest::rstest;

use crate::tests::codegen::{compile_to_wasm, with_db};

/// A program with enough real work that `-O2` actually rewrites its code —
/// FBs, loops, arithmetic. RETAIN and a CONFIGURATION so every
/// memory-describing section is populated.
const SRC: &str = r#"
FUNCTION_BLOCK Ramp
VAR_INPUT target : DINT; rate : DINT; END_VAR
VAR_OUTPUT value : DINT; END_VAR
VAR i : DINT; acc : DINT; END_VAR
    acc := 0;
    FOR i := 0 TO 50 DO
        acc := acc + rate;
        IF acc > target THEN
            acc := target;
        END_IF;
    END_FOR;
    value := acc;
END_FUNCTION_BLOCK

PROGRAM Main
VAR RETAIN n : DINT; keep : DINT; END_VAR
VAR r1 : Ramp; r2 : Ramp; total : DINT; k : DINT; END_VAR
    n := n + 1;
    r1(target := 1000, rate := 7);
    r2(target := 500, rate := 3);
    total := 0;
    FOR k := 0 TO 20 DO
        total := total + r1.value + r2.value;
    END_FOR;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
"#;

/// Every custom section a module carries, by name, as raw bytes.
fn custom_sections(wasm: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(c)) = payload {
            out.insert(c.name().to_string(), c.data().to_vec());
        }
    }
    out
}

/// The raw bytes of the code section, or empty if absent.
fn code_section(wasm: &[u8]) -> Vec<u8> {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CodeSectionStart { range, .. }) = payload {
            return wasm[range.start..range.end].to_vec();
        }
    }
    Vec::new()
}

/// Optimize with the real wasm-opt, or return `None` when it is unavailable so
/// the test skips honestly instead of comparing a module to itself. A silent
/// no-op optimizer would make every assertion below pass vacuously.
fn optimize(wasm: &[u8]) -> Option<Vec<u8>> {
    // Never reach for the network in a test: only use a wasm-opt already
    // present. `find` with the download refused returns None when absent.
    // SAFETY: single-threaded test setup, no other thread reads the env here.
    unsafe { std::env::set_var("RK_NO_DOWNLOAD", "1") };
    let optimized = rk::compiler::optimize_wasm(wasm.to_vec(), Some("2"), false);
    // `optimize_wasm` returns the input unchanged when it cannot optimize;
    // treat an unchanged code section as "no optimizer ran".
    if code_section(&optimized) == code_section(wasm) {
        return None;
    }
    Some(optimized)
}

/// The sections that describe MEMORY — decided in MIR, before wasm-opt — must
/// be byte-identical across the plain and optimized builds. This is the whole
/// basis for carrying live state across a debug-attach swap.
#[rstest]
fn optimization_preserves_every_memory_describing_section(mut with_db: RootDatabase) {
    let plain = compile_to_wasm(&mut with_db, SRC);
    let Some(opt) = optimize(&plain) else {
        eprintln!("SKIP: no wasm-opt available; memory-invariance not exercised");
        return;
    };

    // Guard against a vacuous pass: the code really was rewritten.
    assert_ne!(
        code_section(&plain),
        code_section(&opt),
        "wasm-opt did not change the code — the test would prove nothing"
    );

    let a = custom_sections(&plain);
    let b = custom_sections(&opt);

    // Every section whose contents are addresses or data offsets, none of
    // which wasm-opt may touch.
    for name in ["debug-symbols", "retain-map", "rk.schedule", "debug-locals"] {
        assert_eq!(
            a.get(name),
            b.get(name),
            "`{name}` changed under optimization — it describes memory and must \
             not depend on code layout, or the debug-attach swap restores state \
             into the wrong addresses"
        );
    }

    // The globals band bounds live in exported globals, not a custom section;
    // the export section carrying them must also survive unchanged.
    assert_eq!(
        globals_bounds(&plain),
        globals_bounds(&opt),
        "the globals band moved under optimization — a monitoring tool's \
         addresses would point at the wrong bytes after a swap"
    );
}

/// The counterpart: `debug-lines` is PC-indexed, so optimization invalidates
/// it. Its bytes may survive (wasm-opt passes custom sections through opaque),
/// but the module is no longer safe to line-step. This asserts the *stale*
/// state exists, so the swap design documents it rather than discovering it in
/// the field — a breakpoint on an optimized build would land on the wrong
/// instruction.
#[rstest]
fn optimization_invalidates_the_line_table(mut with_db: RootDatabase) {
    let plain = compile_to_wasm(&mut with_db, SRC);
    let Some(opt) = optimize(&plain) else {
        eprintln!("SKIP: no wasm-opt available");
        return;
    };

    let a = custom_sections(&plain);
    let b = custom_sections(&opt);

    // wasm-opt passes unknown custom sections through unchanged, so the line
    // table's BYTES are identical...
    assert_eq!(
        a.get("debug-lines"),
        b.get("debug-lines"),
        "wasm-opt is expected to pass `debug-lines` through opaquely"
    );
    // ...while the code they index HAS moved. Identical table + changed code =
    // a table that now points at the wrong PCs. This is exactly why line
    // stepping requires the unoptimized build.
    assert_ne!(
        code_section(&plain),
        code_section(&opt),
        "the code did not move, so the line table is not actually stale here"
    );
}

/// Read the exported `globals_base` / `globals_size` as (base, size). These
/// bound the band a monitoring tool reads, so they must be swap-stable.
fn globals_bounds(wasm: &[u8]) -> (Option<i64>, Option<i64>) {
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, wasm).expect("valid module");
    let mut store = wasmtime::Store::new(&engine, ());
    let memory =
        wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).expect("memory");
    let mut linker = wasmtime::Linker::new(&engine);
    linker
        .define(&store, "env", "memory", memory)
        .expect("define memory");
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("instantiate");
    let read = |store: &mut wasmtime::Store<()>, name: &str| {
        instance
            .get_global(&mut *store, name)
            .and_then(|g| g.get(store).i32())
            .map(|v| v as i64)
    };
    let base = read(&mut store, "globals_base");
    let size = read(&mut store, "globals_size");
    (base, size)
}

/// The two artifacts of Model B, from one source: Release omits exactly the
/// stepping tier and keeps everything else byte-identical.
///
/// The absence IS the interface — a runtime detects which artifact it was
/// handed by these sections' presence, so a release build that quietly kept
/// its line table would make every "is this steppable?" answer a lie, and one
/// that dropped the symbol table would break watching a plant.
#[rstest]
fn release_omits_the_stepping_tier_and_nothing_else(mut with_db: RootDatabase) {
    use crate::tests::codegen::compile_to_mir_and_wasm;

    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, SRC);
    let debug = wasm_codegen::generate_wasm_profile(&with_db, &mir, wasm_codegen::Profile::Debug)
        .finish();
    let release =
        wasm_codegen::generate_wasm_profile(&with_db, &mir, wasm_codegen::Profile::Release)
            .finish();

    let debug_sections = custom_sections(&debug);
    let release_sections = custom_sections(&release);

    // The stepping tier: present in Debug, absent in Release.
    for stepping in [
        debug_format::DEBUG_FUNCTIONS_SECTION,
        debug_format::DEBUG_LINES_SECTION,
        debug_format::DEBUG_LOCALS_SECTION,
    ] {
        assert!(
            debug_sections.contains_key(stepping),
            "the debug artifact carries `{stepping}`"
        );
        assert!(
            !release_sections.contains_key(stepping),
            "the release artifact must NOT carry `{stepping}`"
        );
    }

    // Everything else — monitoring, retain, schedule, tests — is in both,
    // byte-identical: the profiles differ in what rides along, never in what
    // the module IS.
    for (name, bytes) in &release_sections {
        if name == "name" {
            continue; // the wasm name section is tooling courtesy, not ours
        }
        assert_eq!(
            debug_sections.get(name),
            Some(bytes),
            "section `{name}` must be byte-identical across profiles"
        );
    }
    assert_eq!(
        code_section(&debug),
        code_section(&release),
        "the CODE is the same in both profiles — a profile is not a compiler mode"
    );

    // The seam the runtime detects the artifact by: has_lines is the
    // steppability answer Meta serves and the debug profile refuses on.
    let debug_info = debug_format::DebugInfo::from_wasm(&debug);
    let release_info = debug_format::DebugInfo::from_wasm(&release);
    assert!(debug_info.has_lines(), "the debug artifact is steppable");
    assert!(
        !release_info.has_lines(),
        "the release artifact reports itself unsteppable"
    );
    assert!(
        !release_info.list_symbols().is_empty(),
        "and still watchable: the symbol table is intact"
    );
}
