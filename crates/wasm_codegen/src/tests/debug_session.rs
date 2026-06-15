//! Native-breakpoint debugger, end-to-end: set a breakpoint by source line on a
//! compiled PLC (no codegen, no DWARF — just `line_to_pc` → a native wasmtime
//! breakpoint), run a scan, and confirm the hit captures a source-level stack.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::debug_session::DebugSession;

/// 0-based source line of the first occurrence of `needle`.
fn row_of(src: &str, needle: &str) -> u32 {
    let byte = src.find(needle).unwrap_or_else(|| panic!("`{needle}` not in source"));
    src[..byte].bytes().filter(|&b| b == b'\n').count() as u32
}

/// A breakpoint set by source line stops the scan at that statement, and the
/// captured stack names the function and points at the line.
#[rstest]
fn breakpoint_captures_source_level_stack(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR count : INT; END_VAR
            count := count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // `DebugSession` is async (the debug handler suspends the wasm fiber, so the
    // store is async-required); the real executor will be the debugger's
    // runtime. Here the test is the top-level consumer, so it blocks at its own
    // boundary with pollster.
    let mut sess = pollster::block_on(DebugSession::load(&wasm)).expect("load debug session");
    let line = row_of(source, "count := count + 1");
    assert!(sess.set_breakpoint(0, line), "breakpoint set at the assignment");

    pollster::block_on(sess.run(1)).expect("scan");

    let hits = sess.hits();
    assert_eq!(hits.len(), 1, "breakpoint hit exactly once in one scan");
    let stack = &hits[0];
    assert!(!stack.is_empty(), "captured a non-empty stack");
    eprintln!("captured stack: {stack:#?}");

    // Innermost frame is Main's body, stopped exactly at the breakpoint line.
    let top = &stack[0];
    assert!(
        top.function.as_deref().is_some_and(|n| n.contains("Main")),
        "innermost frame is Main's body, got {stack:?}"
    );
    assert_eq!(
        top.source.as_ref().map(|s| s.line),
        Some(line),
        "stopped exactly at the breakpoint line"
    );
}
