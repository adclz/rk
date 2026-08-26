//! Tests for CONFIGURATION / TASK schedule lowering (Phase 1, cooperative).

use crate::tests::codegen::{compile_to_mir_and_wasm, compile_to_mir_and_wasm_expecting, with_db};
use rstest::*;

/// Two RESOURCEs no longer collapse into one anonymous task list: each task
/// carries the resource that declares it, which is what a deployment binds to
/// an execution unit. Before this, `lower_schedule` flattened resources away
/// on its first statement and nothing downstream could tell them apart.
#[rstest]
fn tasks_carry_the_resource_that_declares_them(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA VAR a : INT; END_VAR a := a + 1; END_PROGRAM
        PROGRAM ProgB VAR b : INT; END_VAR b := b + 1; END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Core0 ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM PA WITH Fast : ProgA;
            END_RESOURCE
            RESOURCE Core1 ON CPU
                TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM PB WITH Slow : ProgB;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    // A second RESOURCE is refused (E0247) — this asserts what MIR keeps of a
    // configuration the compiler will not let a user deploy.
    let (mir, _wasm) = compile_to_mir_and_wasm_expecting(&mut with_db, source, &["E0247"]);
    let sched = mir.schedule.as_ref().expect("a schedule");
    let pairs: Vec<(String, String)> = sched
        .tasks
        .iter()
        .map(|t| {
            (
                t.resource.text(&with_db).to_string(),
                t.name.text(&with_db).to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("Core0".to_string(), "Fast".to_string()),
            ("Core1".to_string(), "Slow".to_string())
        ]
    );
}

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

/// Codegen carries the schedule as the `rk.schedule` custom section — task
/// names, resources, periods and priorities, and per-instance the export to
/// call with its address — and emits none of the old scheduler machinery
/// (`__task_<i>` entries, `__task_count`/`__common_ticktime_ns` globals).
#[rstest]
fn config_emits_the_schedule_manifest(mut with_db: db::RootDatabase) {
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

    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut manifest = None;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == debug_format::SCHEDULE_SECTION
        {
            manifest =
                Some(debug_format::ScheduleManifest::from_msgpack(reader.data()).expect("decodes"));
        }
    }
    let manifest = manifest.expect("module carries an `rk.schedule` section");

    assert_eq!(manifest.version, debug_format::SCHEDULE_VERSION);
    assert_eq!(manifest.common_ticktime_ns, 10_000_000, "GCD(10ms, 20ms)");

    let shape: Vec<String> = manifest
        .tasks
        .iter()
        .flat_map(|t| {
            std::iter::once(format!(
                "{}::{} every {} ticks, priority {:?}",
                t.resource, t.name, t.period_ticks, t.priority
            ))
            .chain(
                t.programs
                    .iter()
                    .map(|p| format!("  {} -> {}", p.instance, p.export)),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            "Res::Fast every 1 ticks, priority Some(1)",
            "  PA -> ProgA$__body__",
            "Res::Slow every 2 ticks, priority Some(2)",
            "  PB -> ProgB$__body__",
        ]
    );

    // The manifest's addresses are MIR's — the layout is owned by the module,
    // the manifest only records it.
    let mir_addrs: Vec<u32> = mir
        .schedule
        .as_ref()
        .expect("schedule")
        .tasks
        .iter()
        .flat_map(|t| t.programs.iter().map(|p| p.instance_addr))
        .collect();
    let manifest_addrs: Vec<u32> = manifest
        .tasks
        .iter()
        .flat_map(|t| t.programs.iter().map(|p| p.instance_addr))
        .collect();
    assert_eq!(manifest_addrs, mir_addrs);

    // The schedule-as-code machinery is gone: no synthesized entries, no
    // metadata globals.
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).expect("valid module");
    for export in module.exports() {
        assert!(
            !export.name().starts_with("__task"),
            "stale scheduler export: {}",
            export.name()
        );
        assert_ne!(export.name(), "__common_ticktime_ns");
    }
}

/// The GVL split lowers whole: VAR_GLOBALs in one file, the RESOURCE in
/// another, and the artifact carries both. Lowering used to take the first
/// CONFIGURATION block it found, so whichever file lost the race contributed
/// nothing — an artifact missing either its globals or its schedule, with no
/// diagnostic.
#[rstest]
fn fragments_in_separate_files_lower_together(mut with_db: db::RootDatabase) {
    use auto_lsp::default::db::BaseDatabase;
    use hir::hir_def::semantic_index::semantic_index;

    let globals = r#"
CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : INT := 7;
    END_VAR
END_CONFIGURATION
"#;
    let machine = r#"
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : INT;
END_VAR
    line_speed := line_speed + 1;
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Cyclic(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Cyclic : Conveyor;
    END_RESOURCE
END_CONFIGURATION
"#;
    crate::tests::utils::add_sources(&mut with_db, &[globals, machine]);
    let files: Vec<_> = with_db.get_files().iter().map(|e| *e.value()).collect();
    let indices: Vec<_> = files.iter().map(|f| semantic_index(&with_db, *f)).collect();
    let module = mir::lower::lower_module::lower_modules(&with_db, &indices).expect("lowers");

    // The globals fragment contributed its variable...
    assert!(
        module.globals_size > 0,
        "the VAR_GLOBAL fragment contributed no memory"
    );
    // ...and the resources fragment contributed the schedule.
    let schedule = module.schedule.as_ref().expect("a schedule");
    assert_eq!(schedule.common_ticktime_ns, 10_000_000);
    let tasks: Vec<_> = schedule
        .tasks
        .iter()
        .map(|t| t.name.text(&with_db).to_string())
        .collect();
    assert_eq!(tasks, ["Cyclic"]);
}

/// Two fragments each contributing a RESOURCE: both reach the schedule, and
/// the base tick is the GCD across BOTH fragments' intervals — the tick math
/// spans the whole configuration, not whichever fragment was lowered first.
#[rstest]
fn both_fragments_contribute_to_one_schedule(mut with_db: db::RootDatabase) {
    use auto_lsp::default::db::BaseDatabase;
    use hir::hir_def::semantic_index::semantic_index;

    let fast = r#"
PROGRAM ProgA VAR a : INT; END_VAR a := a + 1; END_PROGRAM

CONFIGURATION Plant
    RESOURCE Fast ON CPU
        TASK Quick(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM PA WITH Quick : ProgA;
    END_RESOURCE
END_CONFIGURATION
"#;
    let slow = r#"
PROGRAM ProgB VAR b : INT; END_VAR b := b + 1; END_PROGRAM

CONFIGURATION Plant
    RESOURCE Slow ON CPU
        TASK Lazy(INTERVAL := T#25ms, PRIORITY := 2);
        PROGRAM PB WITH Lazy : ProgB;
    END_RESOURCE
END_CONFIGURATION
"#;
    crate::tests::utils::add_sources(&mut with_db, &[fast, slow]);
    let files: Vec<_> = with_db.get_files().iter().map(|e| *e.value()).collect();
    let indices: Vec<_> = files.iter().map(|f| semantic_index(&with_db, *f)).collect();
    let module = mir::lower::lower_module::lower_modules(&with_db, &indices).expect("lowers");

    let schedule = module.schedule.as_ref().expect("a schedule");
    assert_eq!(
        schedule.common_ticktime_ns, 5_000_000,
        "GCD(10ms, 25ms) — a base tick neither fragment could compute alone"
    );

    let mut shape: Vec<String> = schedule
        .tasks
        .iter()
        .map(|t| {
            format!(
                "{}::{} every {} ticks",
                t.resource.text(&with_db),
                t.name.text(&with_db),
                t.period_ticks
            )
        })
        .collect();
    shape.sort();
    assert_eq!(
        shape,
        ["Fast::Quick every 2 ticks", "Slow::Lazy every 5 ticks"]
    );

    // Both fragments' programs got instance memory.
    let instances: Vec<String> = schedule
        .tasks
        .iter()
        .flat_map(|t| t.programs.iter().map(|p| p.inst_name.text(&with_db).to_string()))
        .collect();
    assert_eq!(instances.len(), 2, "one instance per fragment: {instances:?}");
}

/// End-to-end: a module scheduling two RESOURCEs must be refused at load.
/// Each RESOURCE is its own execution unit, so honouring both means
/// concurrent tasks over shared globals — and this runtime scans on one
/// thread. Interleaving them there would just be called concurrency.
#[rstest]
fn a_compiled_two_resource_module_is_refused_at_load(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        PROGRAM Prog
        VAR n : INT; END_VAR
            n := n + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Core0 ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM PA WITH Fast : Prog;
            END_RESOURCE
            RESOURCE Core1 ON CPU
                TASK Slow(INTERVAL := T#50ms, PRIORITY := 2);
                PROGRAM PB WITH Slow : Prog;
            END_RESOURCE
        END_CONFIGURATION
    "#;

    // The compiler refuses this configuration too (E0247), so the load-time
    // refusal below is the second line: it holds for a module that reached the
    // runtime some other way.
    let (_mir, wasm) = compile_to_mir_and_wasm_expecting(&mut with_db, source, &["E0247"]);
    let Err(err) = Plc::load(&wasm, Config::default()) else {
        panic!("a two-resource module must not load");
    };
    let err = format!("{err:#}");
    assert!(
        err.contains("Core0") && err.contains("Core1"),
        "the refusal should name the resources, got: {err}"
    );
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
                retain_path: Some(path.clone()),
                program_path: None,
                ..Config::default()
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
                retain_path: Some(path.clone()),
                program_path: None,
                ..Config::default()
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
