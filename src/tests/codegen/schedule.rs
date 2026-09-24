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
    // A second RESOURCE is refused (E1403) — this asserts what MIR keeps of a
    // configuration the compiler will not let a user deploy.
    let (mir, _wasm) = compile_to_mir_and_wasm_expecting(&mut with_db, source, &["E1403"]);
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

/// A period is as wide as the INTERVAL it comes from. The manifest carried it
/// as 32 bits while the tick counter is 64: a period past 2^32 base ticks
/// wrapped, and one landing on exactly 2^32 became 0, which the runtime read
/// as "every tick". This task, declared every 49 days beside a 1ms one, ran
/// every millisecond, from a program `check` called clean.
#[rstest]
fn a_period_past_32_bits_is_carried_whole(mut with_db: db::RootDatabase) {
    use crate::tests::codegen::TestPlc;

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
                TASK Fast(INTERVAL := T#1ms, PRIORITY := 1);
                TASK Rare(INTERVAL := T#49d17h2m47s296ms, PRIORITY := 2);
                PROGRAM PA WITH Fast : ProgA;
                PROGRAM PB WITH Rare : ProgB;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let schedule = mir.schedule.as_ref().expect("schedule");
    let rare = &schedule.tasks[1];
    assert_eq!(rare.name.text(&with_db).as_str(), "Rare");
    assert_eq!(rare.period_ticks, 1 << 32, "49d17h2m47s296ms is exactly 2^32 ticks of 1ms");

    // Retain band holds [a, b] (declaration order), 4 bytes each. Eight ticks:
    // Fast fires on every one, Rare on tick 0 and not again for 49 days.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(8).expect("scans");
    let r = plc.read_retain();
    let a = i32::from_le_bytes(r[0..4].try_into().unwrap());
    let b = i32::from_le_bytes(r[4..8].try_into().unwrap());
    assert_eq!((a, b), (8, 1), "(8, 8) means the period wrapped to every tick");
}

/// The point of the instance model: two instances of the SAME program type have
/// independent state. `P1` (fast) and `P2` (slow) share the `Counter` type but
/// run at different rates, so their retained counters diverge.
#[rstest]
fn two_instances_of_one_program_type_are_independent(mut with_db: db::RootDatabase) {
    use crate::tests::codegen::TestPlc;

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

    let mut plc = TestPlc::load(&wasm).expect("load");
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

/// A program instance's connections: each input is copied from its source
/// (a direct variable, a global or a constant) before the instance's body
/// runs, and each output to its sink after, for that instance only.
#[rstest]
fn a_program_connection_copies_in_and_out(mut with_db: db::RootDatabase) {
    use debug_format::{DebugInfo, VarValue};
    let source = r#"
        PROGRAM F
        VAR_INPUT x1 : BOOL; x2 : UINT; END_VAR
        VAR_OUTPUT y1 : UINT; END_VAR
        VAR n : UINT; END_VAR
            IF x1 THEN n := n + x2; END_IF;
            y1 := n;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL w : UINT := 5; total AT %QW0 : UINT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : F(x1 := %IX0.0, x2 := w, y1 => total);
                PROGRAM P2 WITH T : F(x1 := TRUE, x2 := 3);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.write_located("%IX0.0", &1i32.to_le_bytes())
        .expect("x1");
    plc.run(2).expect("scans");
    let total = u16::from_le_bytes(
        plc.read_located("%QW0").expect("total")[..2]
            .try_into()
            .unwrap(),
    );
    assert_eq!(total, 10, "P1 added `w` twice and copied its output out");
    let loc = info.resolve("P2.y1").expect("P2.y1");
    assert_eq!(
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap()),
        VarValue::U16(6),
        "P2's own connections: TRUE and 3"
    );
}

/// A connection to part of a wider address reads and writes those bits or
/// bytes of its cell, and leaves the rest of the cell as it was.
#[rstest]
fn a_connection_to_part_of_a_wider_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM F
        VAR_INPUT b : BOOL; lo : BYTE; END_VAR
        VAR_OUTPUT q : BOOL; hi : BYTE; END_VAR
            q := b;
            hi := lo;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL inw AT %IW0 : WORD; outw AT %QW0 : WORD := 16#0001; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : F(b := %IX0.3, lo := %IB1, q => %QX0.1, hi => %QB1);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.write_located("%IW0", &0x2A08u16.to_le_bytes())
        .expect("inw");
    plc.run(1).expect("scan");
    let out = u16::from_le_bytes(
        plc.read_located("%QW0").expect("outw")[..2]
            .try_into()
            .unwrap(),
    );
    assert_eq!(out, 0x2A03, "bit 1 and byte 1 written, bit 0 kept");
}

/// A function block associated with a task runs under that task alone: it
/// counts every FAST tick, while the program holding it runs every other.
#[rstest]
fn a_function_block_runs_under_its_own_task(mut with_db: db::RootDatabase) {
    use debug_format::{DebugInfo, VarValue};
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : INT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        PROGRAM G
        VAR fb1 : Counter; seen : INT; END_VAR
            seen := fb1.n;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK FAST(INTERVAL := T#10ms, PRIORITY := 1);
                TASK SLOW(INTERVAL := T#20ms, PRIORITY := 2);
                PROGRAM P2 WITH SLOW : G(fb1 WITH FAST);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.run(4).expect("ticks");
    let read = |path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    assert_eq!(read("P2.fb1.n"), VarValue::I16(4), "FAST ran it every tick");
    assert_eq!(
        read("P2.seen"),
        VarValue::I16(3),
        "SLOW ran P2 on ticks 0 and 2"
    );
}
