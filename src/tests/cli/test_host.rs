// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test` runs the component, as a host runs it: WASI's clocks from
//! wasmtime-wasi, the zone from the host's own, and every other import a
//! trap.

use crate::tests::codegen::{compile_tests, compile_to_mir_and_wasm_as_built, with_db};
use rstest::rstest;

/// The clocks, declared as `Std.Timers` declares them.
const CLOCKS: &str = r#"
    {extern 'wasi:clocks/monotonic-clock@0.3.0' 'now'}
    FUNCTION MonotonicNow : LINT
    END_FUNCTION

    {extern 'wasi:clocks/system-clock@0.3.0' 'now'}
    FUNCTION WallNow
    VAR_OUTPUT seconds : LINT; nanoseconds : UDINT; END_VAR
    END_FUNCTION

    {extern 'wasi:clocks/timezone@0.3.0' 'utc-offset'}
    FUNCTION UtcOffset
    VAR_INPUT seconds : LINT; nanoseconds : UDINT; END_VAR
    VAR_OUTPUT known : BOOL; offset : LINT; END_VAR
    END_FUNCTION
"#;

/// The tests read the clocks the component imports: the wall clock is
/// today's, the monotonic clock never goes back, and the zone, when the
/// machine has one, is less than a day from UTC.
#[rstest]
fn the_tests_read_wasi_s_clocks(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CLOCKS}
        {{test}}
        FUNCTION test_the_wall_clock_is_today
        VAR seconds : LINT; END_VAR
            WallNow(seconds => seconds);
            IF seconds < LINT#1735689600 THEN
                __RAISE('the wall clock is before 2025');
            END_IF;
        END_FUNCTION

        {{test}}
        FUNCTION test_the_monotonic_clock_never_goes_back
        VAR a : LINT; b : LINT; END_VAR
            a := MonotonicNow();
            b := MonotonicNow();
            IF b < a THEN
                __RAISE('the monotonic clock went back');
            END_IF;
        END_FUNCTION

        {{test}}
        FUNCTION test_the_zone_is_less_than_a_day_away
        VAR seconds : LINT; nanoseconds : UDINT; known : BOOL; offset : LINT; END_VAR
            WallNow(seconds => seconds, nanoseconds => nanoseconds);
            UtcOffset(seconds := seconds, nanoseconds := nanoseconds, known => known, offset => offset);
            IF known AND (offset >= LINT#86400000000000
                OR offset <= LINT#-86400000000000) THEN
                __RAISE('the zone is a day away');
            END_IF;
        END_FUNCTION
    "#
    );
    let build = compile_tests(&mut with_db, &source);
    let records =
        rk::test_host::run_each(&build.component, None, None, |_| {}).expect("the tests run");
    let failed: Vec<_> = records
        .iter()
        .filter(|record| record.reason.is_some())
        .map(|record| (&record.name, &record.reason))
        .collect();
    assert_eq!(records.len(), 3);
    assert!(failed.is_empty(), "{failed:?}");
}

/// A host function nothing wires traps when a test calls it, and only that
/// test fails.
#[rstest]
fn an_unwired_host_function_traps_the_test_that_calls_it(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'modbus' 'read'}
        FUNCTION Read : INT
        VAR_INPUT address : UINT; END_VAR
        END_FUNCTION

        {test}
        FUNCTION test_reads
        VAR r : INT; END_VAR
            r := Read(address := 1);
        END_FUNCTION

        {test}
        FUNCTION test_does_not
        END_FUNCTION
    "#;
    let build = compile_tests(&mut with_db, source);
    let records =
        rk::test_host::run_each(&build.component, None, None, |_| {}).expect("the tests run");
    let by_name = |name: &str| {
        records
            .iter()
            .find(|record| record.name == name)
            .unwrap_or_else(|| panic!("no record for {name}"))
    };
    let reads = by_name("test_reads");
    assert!(
        reads
            .reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("trapped: ") && reason.contains("modbus")),
        "{:?}",
        reads.reason
    );
    assert_eq!(by_name("test_does_not").reason, None);
}

/// What placing a component's trap rests on: wasmtime counts a frame's
/// offset from the start of the file that ran, so the same trap is as far
/// into the component as into the core module, plus where the component
/// holds the module. `rk test` takes that off; read as the module's, the
/// offsets placed a caller on a neighbouring line.
#[rstest]
fn a_component_s_backtrace_counts_from_the_component_s_start(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Truncate : DINT
        VAR_INPUT x : REAL; END_VAR
            {wasm Truncate 'trunc_f32_s' (params x) (result Truncate)}
        END_FUNCTION

        {test}
        FUNCTION test_truncate
        VAR r : DINT; END_VAR
            r := Truncate(1.0E30);
        END_FUNCTION
    "#;
    let (mir, core) = compile_to_mir_and_wasm_as_built(&mut with_db, source);
    let interfaces = rk::component::interfaces(&with_db, &mir).unwrap();
    let component = rk::component::wrap(&core, "plant", &interfaces)
        .unwrap()
        .bytes;
    let module_start = wasmparser::Parser::new(0)
        .parse_all(&component)
        .find_map(|payload| match payload {
            Ok(wasmparser::Payload::ModuleSection {
                unchecked_range, ..
            }) => Some(unchecked_range.start),
            _ => None,
        })
        .expect("the component wraps a module");
    assert!(module_start > 0);

    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let offsets = |err: wasmtime::Error| -> Vec<(u32, usize)> {
        err.downcast_ref::<wasmtime::WasmBacktrace>()
            .expect("a backtrace")
            .frames()
            .iter()
            .map(|frame| {
                (
                    frame.func_index(),
                    frame.module_offset().expect("an offset"),
                )
            })
            .collect()
    };

    let module = wasmtime::Module::new(&engine, &core).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let test = instance
        .get_typed_func::<(), i32>(&mut store, "test_truncate")
        .unwrap();
    let alone = offsets(test.call(&mut store, ()).expect_err("the test traps"));
    assert_eq!(alone.len(), 2, "{alone:?}");

    let loaded = wasmtime::component::Component::new(&engine, &component).unwrap();
    let linker = wasmtime::component::Linker::<()>::new(&engine);
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &loaded).unwrap();
    let control = instance
        .get_export_index(&mut store, None, debug_format::wit::CONTROL_INTERFACE)
        .unwrap();
    let run_test = instance
        .get_export_index(&mut store, Some(&control), "run-test")
        .unwrap();
    let run_test = instance
        .get_typed_func::<(u32,), (Result<(), rk::control::TestError>,)>(&mut store, &run_test)
        .unwrap();
    let wrapped = offsets(run_test.call(&mut store, (0,)).expect_err("the test traps"));
    // The innermost frames are the test's; the interface's own is past them.
    let wrapped: Vec<(u32, usize)> = wrapped[..alone.len()]
        .iter()
        .map(|(function, offset)| (*function, offset - module_start))
        .collect();
    assert_eq!(wrapped, alone);
}
