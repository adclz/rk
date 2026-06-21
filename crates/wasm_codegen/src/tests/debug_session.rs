//! Native-breakpoint debugger, end-to-end: set a breakpoint by source line on a
//! compiled PLC (no codegen, no DWARF — just `line_to_pc` → a native wasmtime
//! breakpoint), run a scan, and confirm the hit captures a source-level stack.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use futures::StreamExt;
use rstest::*;
use runtime::debug::VarValue;
use runtime::debug_session::{DebugCommand, DebugSession, StepKind, Stop};

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

/// Interactive mode genuinely *halts*: the scan does not finish until the UI
/// sends `Continue`. We drive the scan and the control channel concurrently and
/// assert the `Stop` arrives (with the right stack) before the scan completes.
#[rstest]
fn interactive_breakpoint_halts_until_continue(mut with_db: db::RootDatabase) {
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
    let line = row_of(source, "count := count + 1");

    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm)
            .await
            .expect("load interactive debug session");
        assert!(sess.set_breakpoint(0, line), "breakpoint set at the assignment");

        // The debugger UI: wait for the breakpoint to fire, inspect, resume.
        let ui = async {
            let stop = ctrl.stops.next().await.expect("a stop at the breakpoint");
            let top = &stop.stack[0];
            assert!(
                top.function.as_deref().is_some_and(|n| n.contains("Main")),
                "parked at Main's body, got {:?}",
                stop.stack
            );
            assert_eq!(
                top.source.as_ref().map(|s| s.line),
                Some(line),
                "parked exactly at the breakpoint line"
            );
            ctrl.commands
                .unbounded_send(DebugCommand::Continue)
                .expect("send continue to the parked session");
            stop
        };

        // `join!` drives both: the scan parks in `handle`, the UI receives the
        // stop and sends `Continue`, then the scan resumes and finishes. If the
        // scan didn't actually halt, the UI would never see a stop.
        let (scan, stop) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes after continue");
        assert_eq!(
            stop.stack[0].source.as_ref().map(|s| s.line),
            Some(line)
        );
    });
}

/// 0-based source line of a stop's innermost frame.
fn top_line(stop: &Stop) -> Option<u32> {
    stop.stack
        .first()
        .and_then(|f| f.source.as_ref())
        .map(|p| p.line)
}

/// A program with a function call, for the stepping tests.
const STEP_SRC: &str = r#"
    FUNCTION Add : INT
    VAR_INPUT a : INT; b : INT; END_VAR
        Add := a + b;
    END_FUNCTION

    PROGRAM Main
    VAR x : INT; y : INT; END_VAR
        x := 1;
        y := Add(x, 2);
        x := y;
    END_PROGRAM

    CONFIGURATION Cfg
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Run WITH T : Main;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// `next` (step over) stops at the next line in the same frame, stepping *over* a
/// call rather than into it.
#[rstest]
fn step_over_skips_a_call(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, STEP_SRC);
    let r1 = row_of(STEP_SRC, "x := 1");
    let r2 = row_of(STEP_SRC, "y := Add");
    let r3 = row_of(STEP_SRC, "x := y");
    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm).await.unwrap();
        assert!(sess.set_breakpoint(0, r1));
        let ui = async {
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(r1), "breakpoint at x := 1");
            ctrl.commands.unbounded_send(DebugCommand::Step(StepKind::Over)).unwrap();
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(r2), "next → y := Add");
            ctrl.commands.unbounded_send(DebugCommand::Step(StepKind::Over)).unwrap();
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(r3), "next steps over the call → x := y");
            ctrl.commands.unbounded_send(DebugCommand::Continue).unwrap();
        };
        let (scan, _) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes");
    });
}

/// `stepIn` descends into a called function, stopping at its first statement.
#[rstest]
fn step_into_enters_a_call(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, STEP_SRC);
    let r2 = row_of(STEP_SRC, "y := Add");
    let ra = row_of(STEP_SRC, "Add := a + b");
    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm).await.unwrap();
        assert!(sess.set_breakpoint(0, r2));
        let ui = async {
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(r2), "breakpoint at the call");
            ctrl.commands.unbounded_send(DebugCommand::Step(StepKind::Into)).unwrap();
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(ra), "stepIn enters Add's body");
            ctrl.commands.unbounded_send(DebugCommand::Continue).unwrap();
        };
        let (scan, _) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes");
    });
}

/// Pause halts a freely-running scan (no breakpoint) at the next source line.
#[rstest]
fn pause_stops_a_running_scan(mut with_db: db::RootDatabase) {
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
    let line = row_of(source, "count := count + 1");
    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm).await.unwrap();
        // No breakpoint — request a pause; the scan stops at the next source line.
        sess.request_pause();
        let ui = async {
            let stop = ctrl.stops.next().await.expect("pause produces a stop");
            assert!(
                stop.stack[0].function.as_deref().is_some_and(|n| n.contains("Main")),
                "paused inside Main, got {:?}",
                stop.stack
            );
            assert_eq!(top_line(&stop), Some(line), "paused at the first statement");
            ctrl.commands.unbounded_send(DebugCommand::Continue).unwrap();
        };
        let (scan, _) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes");
    });
}

/// The stop snapshot carries the program's variables with their current values.
#[rstest]
fn stop_snapshots_variables(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR count : INT; flag : BOOL; END_VAR
            count := count + 1;
            flag := TRUE;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let line = row_of(source, "flag := TRUE"); // after the increment, so count == 1
    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm).await.unwrap();
        assert!(sess.set_breakpoint(0, line));
        let ui = async {
            let stop = ctrl.stops.next().await.expect("a stop");
            let count = stop
                .variables
                .iter()
                .find(|(n, _, _)| n.ends_with("count"))
                .expect("count in the snapshot");
            assert_eq!(count.1, VarValue::I16(1), "count == 1 after the first increment");
            assert!(!count.2, "a program variable is not flagged global");
            ctrl.commands.unbounded_send(DebugCommand::Continue).unwrap();
        };
        let (scan, _) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes");
    });
}

/// `stepOut` runs the rest of the current function and stops back in the caller.
#[rstest]
fn step_out_returns_to_caller(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, STEP_SRC);
    let ra = row_of(STEP_SRC, "Add := a + b");
    let r2 = row_of(STEP_SRC, "y := Add");
    pollster::block_on(async {
        let (mut sess, mut ctrl) = DebugSession::load_interactive(&wasm).await.unwrap();
        assert!(sess.set_breakpoint(0, ra)); // inside Add
        let ui = async {
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(ra), "breakpoint inside Add");
            ctrl.commands.unbounded_send(DebugCommand::Step(StepKind::Out)).unwrap();
            assert_eq!(top_line(&ctrl.stops.next().await.unwrap()), Some(r2), "stepOut returns to the caller");
            ctrl.commands.unbounded_send(DebugCommand::Continue).unwrap();
        };
        let (scan, _) = futures::join!(sess.run(1), ui);
        scan.expect("scan completes");
    });
}
