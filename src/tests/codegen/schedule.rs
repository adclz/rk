//! Tests for CONFIGURATION / TASK schedule lowering (Phase 1, cooperative).

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use rstest::*;

/// A CONFIGURATION with two cyclic tasks at different rates lowers to a
/// multi-rate schedule: base tick = GCD of intervals, per-task period in ticks,
/// tasks sorted most-urgent-first.
#[rstest]
fn config_lowers_to_a_multi_rate_schedule(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR a : INT; END_VAR
            a := a + 1;
        END_PROGRAM

        PROGRAM ProgB
        VAR b : INT; END_VAR
            b := b + 2;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM PA WITH Fast : ProgA;
                PROGRAM PB WITH Slow : ProgB;
            END_RESOURCE
        END_CONFIGURATION
    "#;

    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let schedule = mir
        .schedule
        .as_ref()
        .expect("a CONFIGURATION yields a schedule");

    // Base tick = GCD(10ms, 20ms) = 10ms.
    assert_eq!(schedule.common_ticktime_ns, 10_000_000);
    assert_eq!(schedule.tasks.len(), 2);

    // Sorted most-urgent-first: Fast (priority 1) then Slow (priority 2).
    let fast = &schedule.tasks[0];
    assert_eq!(fast.name.text(&with_db).as_str(), "Fast");
    assert_eq!(fast.priority, Some(1));
    assert_eq!(fast.period_ticks, 1, "10ms / 10ms base = 1 tick");
    assert_eq!(fast.programs.len(), 1);

    let slow = &schedule.tasks[1];
    assert_eq!(slow.name.text(&with_db).as_str(), "Slow");
    assert_eq!(slow.priority, Some(2));
    assert_eq!(slow.period_ticks, 2, "20ms / 10ms base = 2 ticks");
    assert_eq!(slow.programs.len(), 1);
}

/// Codegen emits the scheduler metadata globals and one callable `__task_<i>`
/// entry per task (cooperative model B).
#[rstest]
fn config_emits_task_entries_and_scheduler_globals(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR a : INT; END_VAR
            a := a + 1;
        END_PROGRAM

        PROGRAM ProgB
        VAR b : INT; END_VAR
            b := b + 2;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM PA WITH Fast : ProgA;
                PROGRAM PB WITH Slow : ProgB;
            END_RESOURCE
        END_CONFIGURATION
    "#;

    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm).expect("valid module");
    let mut store = wasmtime::Store::new(&engine, ());
    let memory =
        wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).expect("memory");
    let mut linker = wasmtime::Linker::new(&engine);
    linker
        .define(&store, "env", "memory", memory)
        .expect("define env.memory");
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("instantiate");

    let read_i32 = |store: &mut wasmtime::Store<()>, name: &str| {
        let g = instance.get_global(&mut *store, name).expect(name);
        g.get(&mut *store).i32().expect("i32 global")
    };
    let read_i64 = |store: &mut wasmtime::Store<()>, name: &str| {
        let g = instance.get_global(&mut *store, name).expect(name);
        g.get(&mut *store).i64().expect("i64 global")
    };

    assert_eq!(read_i32(&mut store, "__task_count"), 2);
    assert_eq!(read_i64(&mut store, "__common_ticktime_ns"), 10_000_000);
    assert_eq!(
        read_i32(&mut store, "__task_0__period"),
        1,
        "Fast = 10ms/10ms"
    );
    assert_eq!(
        read_i32(&mut store, "__task_1__period"),
        2,
        "Slow = 20ms/10ms"
    );

    // Both task entries exist and run their programs without trapping.
    for entry in ["__task_0", "__task_1"] {
        instance
            .get_typed_func::<(), ()>(&mut store, entry)
            .unwrap_or_else(|_| panic!("entry {entry}"))
            .call(&mut store, ())
            .unwrap_or_else(|e| panic!("running {entry}: {e}"));
    }
}

/// End-to-end: a two-rate CONFIGURATION driven by the `runtime` scheduler runs
/// each task at its rate (fast every tick, slow every other) and retained
/// counters persist across a power cycle.
#[rstest]
fn scheduler_runs_tasks_at_their_rates_and_persists(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        PROGRAM ProgA
        VAR RETAIN a : INT; END_VAR
            a := a + 1;
        END_PROGRAM

        PROGRAM ProgB
        VAR RETAIN b : INT; END_VAR
            b := b + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM PA WITH Fast : ProgA;
                PROGRAM PB WITH Slow : ProgB;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let path = std::env::temp_dir().join(format!("rk_sched_{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&path);

    // Retain band holds [a, b] (declaration order), 4 bytes each.
    let read_ab = |plc: &Plc| -> (i32, i32) {
        let r = plc.read_retain();
        (
            i32::from_le_bytes(r[0..4].try_into().unwrap()),
            i32::from_le_bytes(r[4..8].try_into().unwrap()),
        )
    };

    // Boot 1: 4 base ticks. Fast (period 1) fires at ticks 0,1,2,3 => a = 4.
    // Slow (period 2) fires at ticks 0,2 => b = 2.
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (boot 1)");
        assert_eq!(plc.common_ticktime_ns(), Some(10_000_000));
        plc.run(4).expect("scans");
        assert_eq!(read_ab(&plc), (4, 2));
        plc.snapshot_retain().expect("snapshot");
    }

    // Boot 2 (power cycle): restore (4,2); the tick phase resets to 0. Two more
    // ticks: Fast fires at 0,1 => a = 6; Slow fires at 0 => b = 3.
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (boot 2)");
        assert_eq!(read_ab(&plc), (4, 2), "retained counters restored");
        plc.run(2).expect("scans");
        assert_eq!(read_ab(&plc), (6, 3));
    }

    std::fs::remove_file(&path).ok();
}

/// The point of the instance model: two instances of the SAME program type have
/// independent state. `P1` (fast) and `P2` (slow) share the `Counter` type but
/// run at different rates, so their retained counters diverge.
#[rstest]
fn two_instances_of_one_program_type_are_independent(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        PROGRAM Counter
        VAR RETAIN n : INT; END_VAR
            n := n + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM P1 WITH Fast : Counter;
                PROGRAM P2 WITH Slow : Counter;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Two independent Counter instances => an 8-byte retain band ([P1.n, P2.n]).
    assert_eq!(mir.retain_size, 8, "two INT instances => 8 bytes");

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(4).expect("scans");

    // Fast ran at ticks 0,1,2,3 (4x); Slow at 0,2 (2x). Same type, distinct state.
    let r = plc.read_retain();
    let p1 = i32::from_le_bytes(r[0..4].try_into().unwrap());
    let p2 = i32::from_le_bytes(r[4..8].try_into().unwrap());
    assert_eq!((p1, p2), (4, 2), "independent per-instance state");
}

/// A module with no CONFIGURATION (e.g. a bare program) has no schedule.
#[rstest]
fn no_configuration_yields_no_schedule(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR x : INT; END_VAR
            x := x + 1;
        END_PROGRAM
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert!(mir.schedule.is_none(), "no CONFIGURATION => no schedule");
}
