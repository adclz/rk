// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Tasks that run on a rising edge of their SINGLE. The module does nothing
//! for it: `rk.schedule` names a bit and the host looks at it, as `TestPlc`
//! and the fuzz host do, each through `debug_format::task_due`.

use crate::tests::codegen::{TestPlc, compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, ScheduleManifest, SingleBit, TaskEntry, VarValue, task_due};
use rstest::*;

/// The module of `source`, with what a host reads.
fn load(
    db: &mut db::RootDatabase,
    source: &str,
) -> (Vec<u8>, ScheduleManifest, DebugInfo, TestPlc) {
    let (_mir, wasm) = compile_to_mir_and_wasm(db, source);
    let section = super::expect_section(&wasm, debug_format::SCHEDULE_SECTION);
    let manifest = ScheduleManifest::from_msgpack(section).expect("decodes");
    let info = DebugInfo::from_wasm(&wasm);
    let plc = TestPlc::load(&wasm).expect("load");
    (wasm, manifest, info, plc)
}

/// The task named `name`.
fn task<'m>(manifest: &'m ScheduleManifest, name: &str) -> &'m TaskEntry {
    manifest
        .tasks
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no task `{name}` in {manifest:?}"))
}

/// A DINT variable's value.
fn count(plc: &TestPlc, info: &DebugInfo, path: &str) -> i32 {
    match plc.read_var(info, path) {
        Some(VarValue::I32(n)) => n,
        other => panic!("`{path}` is {other:?}"),
    }
}

/// A task runs once for each rising edge of its SINGLE: not while it stands
/// at TRUE, and not when it falls. The schedule names the global's own byte,
/// and with no period anywhere it has no tick to give.
#[rstest]
fn a_single_runs_its_task_once_per_rising_edge(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := go, PRIORITY := 1);
                PROGRAM P1 WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(manifest.common_ticktime_ns, 0, "no period, no tick");
    let ev = task(&manifest, "Ev");
    assert_eq!(ev.period_ticks, 0);
    assert_eq!(
        ev.single,
        Some(SingleBit {
            addr: info.resolve("go").expect("go").address,
            bit: 0
        })
    );

    plc.run(3).expect("scans");
    assert_eq!(count(&plc, &info, "P1.n"), 0, "FALSE from the start");

    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.run(3).expect("scans");
    assert_eq!(count(&plc, &info, "P1.n"), 1, "held TRUE runs it once");

    plc.write_var(&info, "go", VarValue::Bool(false)).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "P1.n"), 1, "a falling edge runs nothing");

    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.run(1).expect("scans");
    assert_eq!(count(&plc, &info, "P1.n"), 2, "each rising edge runs it");
}

/// Beside a period: the task runs on its period while SINGLE stands at FALSE,
/// not at all while it stands at TRUE past its edge, and on the period's own
/// phase once it falls. An edge on a due tick is one run, and an edge between
/// two due ticks is one more.
#[rstest]
fn a_single_stops_the_period_while_it_stands_at_true(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK Tick(INTERVAL := T#10ms, PRIORITY := 0);
                TASK Both(SINGLE := go, INTERVAL := T#20ms, PRIORITY := 1);
                PROGRAM Clock WITH Tick : Count;
                PROGRAM P1 WITH Both : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(manifest.common_ticktime_ns, 10_000_000);
    assert_eq!(task(&manifest, "Both").period_ticks, 2);

    // Each scan: what SINGLE is set to before it, and the runs so far after.
    // Ticks 0, 2, 4, ... are the period's.
    let scans = [
        (false, 1), // tick 0: due
        (false, 1), // tick 1
        (false, 2), // tick 2: due
        (true, 3),  // tick 3: an edge between two due ticks
        (true, 3),  // tick 4: due, and SINGLE stands at TRUE
        (true, 3),  // tick 5
        (false, 4), // tick 6: due, and SINGLE has fallen
        (false, 4), // tick 7
        (true, 5),  // tick 8: due and an edge, one run
        (true, 5),  // tick 9
        (true, 5),  // tick 10: due, and SINGLE still stands at TRUE
    ];
    for (tick, (level, runs)) in scans.into_iter().enumerate() {
        plc.write_var(&info, "go", VarValue::Bool(level)).unwrap();
        plc.scan().expect("scan");
        assert_eq!(count(&plc, &info, "P1.n"), runs, "tick {tick}");
    }
}

/// A SINGLE already TRUE at the first look counts as a rising edge, as for
/// `R_TRIG`: one run, and none after it.
#[rstest]
fn a_single_true_at_the_first_look_is_a_rising_edge(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL := TRUE; END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := go, PRIORITY := 1);
                PROGRAM P1 WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, _manifest, info, mut plc) = load(&mut with_db, source);
    plc.run(3).expect("scans");
    assert_eq!(count(&plc, &info, "P1.n"), 1);
}

/// What a task's period comes to: a zero INTERVAL is no period and has no say
/// in the tick, and a constant FALSE leaves the period as it was and names no
/// bit at all.
#[rstest]
fn the_period_of_a_task_with_a_single(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            VAR_GLOBAL CONSTANT off : BOOL := FALSE; END_VAR
            RESOURCE Res ON CPU
                TASK Slow(INTERVAL := T#30ms, PRIORITY := 0);
                TASK Fast(SINGLE := FALSE, INTERVAL := T#20ms, PRIORITY := 1);
                TASK Named(SINGLE := off, INTERVAL := T#40ms, PRIORITY := 2);
                TASK Edge(SINGLE := go, INTERVAL := T#0s, PRIORITY := 3);
                PROGRAM A WITH Slow : Count;
                PROGRAM B WITH Fast : Count;
                PROGRAM C WITH Named : Count;
                PROGRAM D WITH Edge : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(manifest.common_ticktime_ns, 10_000_000, "GCD of 30, 20, 40");
    assert_eq!(task(&manifest, "Slow").period_ticks, 3);
    assert_eq!(task(&manifest, "Fast").period_ticks, 2);
    assert_eq!(task(&manifest, "Fast").single, None);
    assert_eq!(task(&manifest, "Named").period_ticks, 4);
    assert_eq!(task(&manifest, "Named").single, None);
    assert_eq!(task(&manifest, "Edge").period_ticks, 0);
    assert!(task(&manifest, "Edge").single.is_some());

    plc.run(5).expect("ticks 0 to 4");
    assert_eq!(count(&plc, &info, "A.n"), 2, "ticks 0 and 3");
    assert_eq!(count(&plc, &info, "B.n"), 3, "ticks 0, 2 and 4");
    assert_eq!(count(&plc, &info, "C.n"), 2, "ticks 0 and 4");
    assert_eq!(count(&plc, &info, "D.n"), 0, "no edge");
}

/// Each place a SINGLE can read, by the symbol that is the same memory: what
/// the schedule names is that symbol's byte, and setting it runs the task. A
/// resource's name changes nothing, and a RETAIN variable is read where the
/// retain band moved it. A subscript naming the CONSTANT `level` reads it,
/// not the type `Level`: names are caseless, and it was E0317.
#[rstest]
#[case::global("go", "go")]
#[case::caseless("GO", "go")]
#[case::through_the_resource("Res.go", "go")]
#[case::element("flags.go", "flags.go")]
#[case::second_element("flags.other", "flags.other")]
#[case::element_through_the_resource("Res.flags.go", "flags.go")]
#[case::nested_element("flags.inner.deep", "flags.inner.deep")]
#[case::element_past_a_wide_one("flags.after", "flags.after")]
#[case::array_of_a_global("list[1]", "list[1]")]
#[case::output("P1.done", "P1.done")]
#[case::output_caseless("p1.DONE", "P1.done")]
#[case::output_through_the_resource("Res.P1.done", "P1.done")]
#[case::output_past_others("P1.last", "P1.last")]
#[case::output_element("P1.status.other", "P1.status.other")]
#[case::output_nested_element("P1.status.inner.deep", "P1.status.inner.deep")]
#[case::output_array("P1.bits[2]", "P1.bits[2]")]
#[case::output_array_last("P1.bits[3]", "P1.bits[3]")]
#[case::output_array_of_a_constant("P1.bits[K]", "P1.bits[2]")]
#[case::output_array_of_a_constant_with_named_values("P1.bits[KN]", "P1.bits[2]")]
#[case::output_array_of_a_named_value("P1.bits[Level#High]", "P1.bits[2]")]
#[case::output_array_of_a_constant_named_like_a_type("P1.bits[level]", "P1.bits[2]")]
#[case::output_grid("P1.grid[1, 0]", "P1.grid[1,0]")]
#[case::output_grid_second_dimension("P1.grid[0, 1]", "P1.grid[0,1]")]
#[case::output_grid_two_brackets("P1.grid[1][1]", "P1.grid[1,1]")]
#[case::output_array_of_arrays("P1.rows[1][0]", "P1.rows[1][0]")]
#[case::output_array_of_structs("P1.many[1].other", "P1.many[1].other")]
#[case::output_of_a_global_instance("timer.q", "timer.q")]
#[case::inherited_output_of_a_global_instance("derived.q", "derived.q")]
#[case::output_of_an_output_instance("P1.made.q", "P1.made.q")]
#[case::output_of_an_instance_in_a_struct("P1.nest.fb.q", "P1.nest.fb.q")]
#[case::retained_global("kept", "kept")]
#[case::retained_output("Held.done", "Held.done")]
#[case::output_of_a_retained_instance("Whole.done", "Whole.done")]
fn a_single_reads_the_byte_of_what_it_names(
    mut with_db: db::RootDatabase,
    #[case] written: &str,
    #[case] symbol: &str,
) {
    let source = format!(
        r#"
        TYPE Inner : STRUCT pad : DINT; deep : BOOL; END_STRUCT; END_TYPE
        TYPE Flags : STRUCT
            go : BOOL;
            other : BOOL;
            wide : LREAL;
            after : BOOL;
            inner : Inner;
        END_STRUCT; END_TYPE
        TYPE Row : ARRAY[0..1] OF BOOL; END_TYPE
        TYPE Level : INT (Low := 1, High := 2); END_TYPE

        FUNCTION_BLOCK Fb
        VAR_OUTPUT pad : LREAL; q : BOOL; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Sub EXTENDS Fb
        VAR_OUTPUT own : BOOL; END_VAR
        END_FUNCTION_BLOCK

        TYPE Nest : STRUCT first : BOOL; fb : Fb; END_STRUCT; END_TYPE

        PROGRAM Source
        VAR_OUTPUT
            done : BOOL;
            status : Flags;
            bits : ARRAY[1..3] OF BOOL;
            grid : ARRAY[0..1, 0..1] OF BOOL;
            rows : ARRAY[0..1] OF Row;
            many : ARRAY[0..1] OF Flags;
            made : Fb;
            nest : Nest;
            last : BOOL;
        END_VAR
        END_PROGRAM

        PROGRAM Retained
        VAR_OUTPUT RETAIN done : BOOL; END_VAR
        END_PROGRAM

        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                go : BOOL;
                flags : Flags;
                list : ARRAY[0..1] OF BOOL;
                timer : Fb;
                derived : Sub;
            END_VAR
            VAR_GLOBAL RETAIN kept : BOOL; END_VAR
            VAR_GLOBAL CONSTANT K : INT := 2; KN : Level := 2; level : Level := Level#High; END_VAR
            RESOURCE Res ON CPU
                TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
                TASK Ev(SINGLE := {written}, PRIORITY := 1);
                PROGRAM P1 WITH Base : Source;
                PROGRAM Held WITH Base : Retained;
                PROGRAM RETAIN Whole WITH Base : Retained;
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    let loc = info.resolve(symbol).expect(symbol);
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: loc.address,
            bit: 0
        }),
        "{written}"
    );
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 0, "{written}");
    plc.write_var(&info, symbol, VarValue::Bool(true)).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 1, "{written}");
}

/// An address gets a cell like one a body names, in its area's band, and the
/// host finds it by its text: nothing else need mention it.
#[rstest]
#[case::input_bit("%IX2.1")]
#[case::input_one_level("%IX2")]
#[case::caseless("%ix3.1")]
#[case::no_size_letter("%I7")]
#[case::output_bit("%QX0.1")]
#[case::marker_bit("%MX0.1")]
fn a_single_reads_an_address_no_body_names(mut with_db: db::RootDatabase, #[case] written: &str) {
    let source = format!(
        r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := {written}, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    let address = written.to_ascii_uppercase().replace("%I7", "%IX7");
    let single = task(&manifest, "Ev").single.expect("a bit");
    assert_eq!(single.bit, 0, "a cell of its own");
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 0);
    plc.write_located(&address, &[1]).expect(&address);
    assert!(single.read(plc.memory_data()).expect("in memory"));
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// A bit of a wider address the workspace mentions is that address's bit: the
/// schedule names the byte of the cell it falls in and its place there, and a
/// host writing the whole word raises it, as one writing the bit does. An
/// address a VAR_GLOBAL is declared at is that VAR_GLOBAL.
#[rstest]
#[case::low_byte("%IX0.3", 0, 3, 0x0008)]
#[case::high_byte("%IX1.2", 1, 2, 0x0400)]
#[case::first_bit("%IX0.0", 0, 0, 0x0001)]
#[case::last_bit("%IX1.7", 1, 7, 0x8000)]
fn a_single_reads_a_bit_of_a_wider_address(
    mut with_db: db::RootDatabase,
    #[case] written: &str,
    #[case] offset: u32,
    #[case] bit: u8,
    #[case] word: u16,
) {
    let source = format!(
        r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL w AT %IW0 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := {written}, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    let cell = info.resolve("w").expect("w").address;
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: cell + offset,
            bit
        })
    );

    plc.write_located("%IW0", &(!word).to_le_bytes()).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 0, "every other bit set");

    plc.write_located("%IW0", &word.to_le_bytes()).unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1, "the word raised it");

    plc.write_located(written, &[0]).unwrap();
    plc.scan().expect("scan");
    plc.write_located(written, &[1]).unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 2, "the address raised it");
}

/// A bit of a variable is the one name besides a part of an address whose bit
/// is not the first of its byte. Memory holds a value little-endian, so the
/// bit counts up through its bytes, whatever holds the variable: a host
/// raising only that bit runs the task, and every other bit set does not.
#[rstest]
#[case::first_bit("status.%X0", "status", 0, 0, 0x0001)]
#[case::low_byte("status.%X3", "status", 0, 3, 0x0008)]
#[case::written_bare("status.3", "status", 0, 3, 0x0008)]
#[case::high_byte("status.%X11", "status", 1, 3, 0x0800)]
#[case::last_bit("status.%X15", "status", 1, 7, 0x8000)]
#[case::caseless("STATUS.%x3", "status", 0, 3, 0x0008)]
#[case::through_the_resource("Res.status.%X3", "status", 0, 3, 0x0008)]
#[case::a_byte("small.%X7", "small", 0, 7, 0x80)]
#[case::a_dword("wide.%X26", "wide", 3, 2, 0x0400_0000)]
#[case::an_lword("long.%X40", "long", 5, 0, 0x0100_0000_0000)]
#[case::an_lword_last("long.63", "long", 7, 7, 0x8000_0000_0000_0000)]
#[case::an_integer("level.%X15", "level", 1, 7, 0x8000)]
#[case::an_element("flags.word.%X9", "flags.word", 1, 1, 0x0200)]
#[case::an_array_element("list[1].%X12", "list[1]", 1, 4, 0x1000)]
#[case::an_output("P1.word.%X9", "P1.word", 1, 1, 0x0200)]
#[case::an_output_written_bare("P1.word.2", "P1.word", 0, 2, 0x0004)]
#[case::an_output_element("P1.status.word.%X10", "P1.status.word", 1, 2, 0x0400)]
#[case::a_function_blocks_output("timer.word.%X8", "timer.word", 1, 0, 0x0100)]
#[case::a_retained_global("kept.%X5", "kept", 0, 5, 0x0020)]
#[case::a_retained_output("Held.word.%X13", "Held.word", 1, 5, 0x2000)]
#[case::a_located_global("sensors.%X10", "sensors", 1, 2, 0x0400)]
#[case::named_values("named.%X10", "named", 1, 2, 0x0400)]
#[case::named_values_on_an_integer("light.3", "light", 0, 3, 0x0008)]
fn a_single_reads_a_bit_of_a_variable(
    mut with_db: db::RootDatabase,
    #[case] written: &str,
    #[case] symbol: &str,
    #[case] offset: u32,
    #[case] bit: u8,
    #[case] raised: u64,
) {
    let source = format!(
        r#"
        TYPE Flags : STRUCT pad : LREAL; word : WORD; END_STRUCT; END_TYPE
        TYPE Bits : WORD (A := 1, B := 2); END_TYPE
        TYPE Light : INT (Red := 1, Green := 2); END_TYPE

        FUNCTION_BLOCK Fb
        VAR_OUTPUT pad : DINT; word : WORD; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Source
        VAR_OUTPUT done : BOOL; word : WORD; status : Flags; END_VAR
        END_PROGRAM

        PROGRAM Retained
        VAR_OUTPUT RETAIN word : WORD; END_VAR
        END_PROGRAM

        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                status : WORD;
                small : BYTE;
                wide : DWORD;
                long : LWORD;
                level : INT;
                flags : Flags;
                list : ARRAY[0..1] OF WORD;
                timer : Fb;
                sensors AT %IW4 : WORD;
                named : Bits := 0;
                light : Light := 0;
            END_VAR
            VAR_GLOBAL RETAIN kept : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
                TASK Ev(SINGLE := {written}, PRIORITY := 1);
                PROGRAM P1 WITH Base : Source;
                PROGRAM Held WITH Base : Retained;
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    let loc = info.resolve(symbol).expect(symbol);
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: loc.address + offset,
            bit
        }),
        "{written}"
    );

    let size = loc.size as usize;
    let others = (!raised).to_le_bytes();
    plc.write_bytes(loc.address, &others[..size]).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(
        count(&plc, &info, "Runs.n"),
        0,
        "{written}: every other bit"
    );

    plc.write_bytes(loc.address, &raised.to_le_bytes()[..size])
        .unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 1, "{written}: this bit alone");
}

/// A bit of a variable located inside a wider address counts up from that
/// address's cell, the variable's place in it and the bit together.
#[rstest]
fn a_single_reads_a_bit_of_a_variable_in_a_wider_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL
                d AT %MD0 : DWORD;
                inner AT %MW1 : WORD;
            END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := inner.%X9, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: info.resolve("d").expect("d").address + 3,
            bit: 1
        })
    );
    plc.write_located("%MD0", &0x0200_0000u32.to_le_bytes())
        .unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// A VAR_GLOBAL located at an address, and the address itself, are one cell,
/// whichever a SINGLE names.
#[rstest]
#[case::by_its_name("sensor")]
#[case::by_its_address("%IX5.2")]
fn a_single_reads_a_located_global(mut with_db: db::RootDatabase, #[case] written: &str) {
    let source = format!(
        r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL sensor AT %IX5.2 : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := {written}, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: info.resolve("sensor").expect("sensor").address,
            bit: 0
        })
    );
    plc.scan().expect("scan");
    plc.write_located("%IX5.2", &[1]).unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// A located VAR_GLOBAL inside a wider address has no cell of its own, and is
/// read from that address's.
#[rstest]
fn a_single_reads_a_global_located_in_a_wider_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL
                w AT %MW2 : WORD;
                sensor AT %MX5.1 : BOOL;
            END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := sensor, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(
        task(&manifest, "Ev").single,
        Some(SingleBit {
            addr: info.resolve("w").expect("w").address + 1,
            bit: 1
        })
    );
    plc.write_located("%MW2", &0x0200u16.to_le_bytes()).unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// The channel VAR_CONFIG gives an instance's variable is an address like any
/// other: a SINGLE reads it where the program writes it through its pointer.
/// So is an address a body writes bare.
#[rstest]
#[case::through_var_config("%QX3.1", "q := ticks >= 2;")]
#[case::written_bare("%QX4.0", "%QX4.0 := ticks >= 2;")]
#[case::marker_written_bare("%MX4.0", "%MX4.0 := ticks >= 2;")]
fn a_single_reads_what_a_program_writes_to_an_address(
    mut with_db: db::RootDatabase,
    #[case] address: &str,
    #[case] write: &str,
) {
    let source = format!(
        r#"
        PROGRAM Source
        VAR q AT %Q* : BOOL; END_VAR
        VAR ticks : DINT; END_VAR
            ticks := ticks + 1;
            {write}
        END_PROGRAM
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_CONFIG
                Res.P1.q AT %QX3.1 : BOOL;
            END_VAR
            RESOURCE Res ON CPU
                TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
                TASK Ev(SINGLE := {address}, PRIORITY := 1);
                PROGRAM P1 WITH Base : Source;
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, &source);
    let single = task(&manifest, "Ev").single.expect("a bit");
    plc.scan().expect("tick 0");
    assert!(!single.read(plc.memory_data()).expect("in memory"));
    assert_eq!(count(&plc, &info, "Runs.n"), 0);
    plc.run(3).expect("ticks 1 to 3");
    assert!(single.read(plc.memory_data()).expect("in memory"));
    assert_eq!(plc.read_located(address).unwrap()[0], 1);
    assert_eq!(count(&plc, &info, "Runs.n"), 1, "the channel rose once");
}

/// A task reads its SINGLE at its turn, so a task ahead of it in the schedule
/// can raise it for the same scan, and one behind it for the next.
#[rstest]
fn a_task_ahead_raises_a_single_for_the_same_scan(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Source
        VAR_OUTPUT done : BOOL; END_VAR
        VAR ticks : DINT; END_VAR
            ticks := ticks + 1;
            done := ticks = 3;
        END_PROGRAM
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Before(SINGLE := P1.done, PRIORITY := 0);
                TASK Base(INTERVAL := T#10ms, PRIORITY := 1);
                TASK After(SINGLE := P1.done, PRIORITY := 2);
                PROGRAM First WITH Before : Count;
                PROGRAM P1 WITH Base : Source;
                PROGRAM Last WITH After : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, _manifest, info, mut plc) = load(&mut with_db, source);
    plc.run(2).expect("ticks 0 and 1");
    assert_eq!(count(&plc, &info, "First.n"), 0);
    assert_eq!(count(&plc, &info, "Last.n"), 0);

    plc.scan().expect("tick 2: P1 raises done");
    assert_eq!(count(&plc, &info, "First.n"), 0, "read before P1 ran");
    assert_eq!(count(&plc, &info, "Last.n"), 1, "read after P1 ran");

    plc.scan().expect("tick 3: P1 lowers done");
    assert_eq!(count(&plc, &info, "First.n"), 1, "read before P1 ran again");
    assert_eq!(count(&plc, &info, "Last.n"), 1);

    plc.run(2).expect("ticks 4 and 5");
    assert_eq!(count(&plc, &info, "First.n"), 1, "one edge, one run");
    assert_eq!(count(&plc, &info, "Last.n"), 1, "one edge, one run");
}

/// A resource named like a global does not hide the global's element, and the
/// global named like the element is another bit: raising it runs nothing.
#[rstest]
fn a_resource_named_like_a_global_reads_the_element(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Flags : STRUCT other : BOOL; go : BOOL; END_STRUCT; END_TYPE
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL flags : Flags; go : BOOL; END_VAR
            RESOURCE flags ON CPU
                TASK Ev(SINGLE := flags.go, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(
        task(&manifest, "Ev").single.map(|s| s.addr),
        Some(info.resolve("flags.go").expect("flags.go").address)
    );
    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 0, "the other bit");
    plc.write_var(&info, "flags.go", VarValue::Bool(true))
        .unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1, "the element");
}

/// A task keeps its own look at what it reads: two tasks on one bit each run
/// once for its edge, each with its period.
#[rstest]
fn two_tasks_read_one_single(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK A(SINGLE := go, PRIORITY := 1);
                TASK B(SINGLE := go, INTERVAL := T#10ms, PRIORITY := 2);
                PROGRAM PA WITH A : Count;
                PROGRAM PB WITH B : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, _manifest, info, mut plc) = load(&mut with_db, source);
    plc.run(2).expect("ticks 0 and 1");
    assert_eq!(count(&plc, &info, "PA.n"), 0);
    assert_eq!(count(&plc, &info, "PB.n"), 2);
    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.run(2).expect("ticks 2 and 3");
    assert_eq!(count(&plc, &info, "PA.n"), 1);
    assert_eq!(count(&plc, &info, "PB.n"), 3);
}

/// A function block runs under a task of its own on that task's edges, as it
/// does on a period. Its body is what the host calls.
#[rstest]
fn a_function_block_runs_on_a_rising_edge(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : DINT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        PROGRAM G
        VAR fb1 : Counter; END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK Base(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Ev(SINGLE := go, PRIORITY := 2);
                PROGRAM P2 WITH Base : G(fb1 WITH Ev);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(task(&manifest, "Ev").period_ticks, 0);
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "P2.fb1.n"), 0);
    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(count(&plc, &info, "P2.fb1.n"), 1);
}

/// A fragment of the configuration may name a global another declares.
#[rstest]
fn a_single_reads_a_global_of_another_fragment(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := go, PRIORITY := 1);
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
        END_CONFIGURATION
    "#;
    let (_wasm, manifest, info, mut plc) = load(&mut with_db, source);
    assert_eq!(
        task(&manifest, "Ev").single.map(|s| s.addr),
        Some(info.resolve("go").expect("go").address)
    );
    plc.write_var(&info, "go", VarValue::Bool(true)).unwrap();
    plc.scan().expect("scan");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// Optimizing keeps what a host reads: the program's writes to the bit, which
/// nothing in the module reads back, and the section naming it.
#[rstest]
fn an_optimized_module_keeps_what_a_single_reads(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Source
        VAR_OUTPUT done : BOOL; END_VAR
        VAR ticks : DINT; END_VAR
            ticks := ticks + 1;
            done := ticks = 2;
        END_PROGRAM
        PROGRAM Count VAR_OUTPUT n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
                TASK Ev(SINGLE := P1.done, PRIORITY := 1);
                PROGRAM P1 WITH Base : Source;
                PROGRAM Runs WITH Ev : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    if !crate::tests::utils::wasm_opt_installed() {
        crate::tests::utils::skip_without_wasm_opt("a SINGLE in an optimized module");
        return;
    }
    let (wasm, manifest, info, _plc) = load(&mut with_db, source);
    let optimized = rk::compiler::optimize_wasm(wasm.clone(), Some("2"), false);
    assert_ne!(optimized, wasm, "wasm-opt rewrote the module");
    let section = super::expect_section(&optimized, debug_format::SCHEDULE_SECTION);
    assert_eq!(ScheduleManifest::from_msgpack(section).unwrap(), manifest);
    let mut plc = TestPlc::load(&optimized).expect("load");
    plc.run(5).expect("scans");
    assert_eq!(count(&plc, &info, "Runs.n"), 1);
}

/// The whole rule: an edge runs the task, a period only while the bit stands
/// at FALSE, and the two together are one run.
#[rstest]
#[case::no_period_no_edge(0, 0, false, false, false)]
#[case::no_period_rising(0, 0, true, false, true)]
#[case::no_period_held(0, 0, true, true, false)]
#[case::no_period_falling(0, 0, false, true, false)]
#[case::due_low(2, 4, false, false, true)]
#[case::due_rising(2, 4, true, false, true)]
#[case::due_held(2, 4, true, true, false)]
#[case::due_falling(2, 4, false, true, true)]
#[case::not_due_low(2, 3, false, false, false)]
#[case::not_due_rising(2, 3, true, false, true)]
#[case::not_due_held(2, 3, true, true, false)]
#[case::not_due_falling(2, 3, false, true, false)]
#[case::every_tick(1, 7, false, false, true)]
#[case::tick_zero(5, 0, false, false, true)]
fn when_a_task_is_due(
    #[case] period_ticks: u64,
    #[case] tick: u64,
    #[case] level: bool,
    #[case] previous: bool,
    #[case] due: bool,
) {
    assert_eq!(task_due(period_ticks, tick, level, previous), due);
}

/// The bit is `(memory[addr] >> bit) & 1`, and one outside the memory is no
/// value at all.
#[rstest]
#[case::lowest(0, 0, Some(true))]
#[case::set(1, 3, Some(true))]
#[case::clear(1, 2, Some(false))]
#[case::highest(1, 7, Some(true))]
#[case::past_the_end(2, 0, None)]
#[case::past_the_byte(0, 8, None)]
fn reading_a_single_bit(#[case] addr: u32, #[case] bit: u8, #[case] value: Option<bool>) {
    let memory = [0b0000_0001u8, 0b1000_1000];
    assert_eq!(SingleBit { addr, bit }.read(&memory), value);
}

/// What the section holds is what a host decodes, for a task with a SINGLE and
/// for one without.
#[rstest]
fn the_schedule_section_round_trips(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE Res ON CPU
                TASK Ev(SINGLE := go, PRIORITY := 1);
                TASK Tick(INTERVAL := T#10ms, PRIORITY := 2);
                PROGRAM A WITH Ev : Count;
                PROGRAM B WITH Tick : Count;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (wasm, manifest, ..) = load(&mut with_db, source);
    assert_eq!(manifest.version, debug_format::SCHEDULE_VERSION);
    let again = ScheduleManifest::from_msgpack(&manifest.to_msgpack()).expect("decodes");
    assert_eq!(again, manifest);
    assert_eq!(
        super::expect_section(&wasm, debug_format::SCHEDULE_SECTION),
        manifest.to_msgpack().as_slice()
    );
    assert!(task(&manifest, "Ev").single.is_some());
    assert_eq!(task(&manifest, "Tick").single, None);
}
