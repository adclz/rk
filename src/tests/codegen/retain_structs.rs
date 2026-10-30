// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What persists of a struct whose elements say `RETAIN` or `NON_RETAIN`:
//! the retain map's ranges, and the values a warm start brings back.

use crate::tests::codegen::{TestPlc, compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// The map's ranges by path and size, in the map's order.
fn ranges(mir: &mir::MirModule) -> Vec<(String, u32)> {
    mir.retain_map
        .ranges
        .iter()
        .map(|r| (r.path.clone(), r.size))
        .collect()
}

/// Save the map's ranges from one PLC and restore them into a fresh one:
/// a power cycle as a host does it.
fn warm_start(mir: &mir::MirModule, wasm: &[u8], plc: &TestPlc) -> TestPlc {
    let saved: Vec<Vec<u8>> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| plc.read_bytes(r.addr, r.size as usize).expect("snapshot"))
        .collect();
    let mut warm = TestPlc::load(wasm).expect("reload");
    for (range, bytes) in mir.retain_map.ranges.iter().zip(&saved) {
        warm.write_bytes(range.addr, bytes).expect("restore");
    }
    warm
}

/// A `RETAIN` element persists where the variable's declaration says
/// nothing, `NON_RETAIN` never does, and an unqualified element follows
/// the declaration: nothing in a `VAR`, everything in a `VAR RETAIN`. An
/// array of the struct answers per element.
#[rstest]
fn an_element_keeps_or_drops_its_value_by_its_own_qualifier(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Counter : STRUCT
                RETAIN count : DINT;
                NON_RETAIN scratch : DINT;
                total : DINT;
            END_STRUCT;
        END_TYPE

        PROGRAM P
        VAR
            c : Counter;
            a : ARRAY[0..1] OF Counter;
        END_VAR
        VAR RETAIN
            r : Counter;
        END_VAR
            c.count := c.count + 1; c.scratch := c.scratch + 1; c.total := c.total + 1;
            r.count := r.count + 1; r.scratch := r.scratch + 1; r.total := r.total + 1;
            a[1].count := a[1].count + 1; a[1].total := a[1].total + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![
            ("P1.a[0].count".to_string(), 4),
            ("P1.a[1].count".to_string(), 4),
            ("P1.c.count".to_string(), 4),
            ("P1.r.count".to_string(), 4),
            ("P1.r.total".to_string(), 4),
        ]
    );

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    let after = [
        ("P1.c.count", 4),
        ("P1.c.scratch", 1),
        ("P1.c.total", 1),
        ("P1.r.count", 4),
        ("P1.r.scratch", 1),
        ("P1.r.total", 4),
        ("P1.a[1].count", 4),
        ("P1.a[1].total", 1),
    ];
    for (path, value) in after {
        assert_eq!(read(&warm, path), VarValue::I32(value), "{path}");
    }
}

/// A global of such a struct, declared by a TYPE or written in place, is
/// in the retain band without saying `RETAIN`, keeping its marked
/// elements, and one that says it keeps every element but the `NON_RETAIN`
/// ones. A global function block instance keeps what its block declares
/// `VAR RETAIN`.
#[rstest]
fn a_global_keeps_what_its_type_marks(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Counter : STRUCT
                RETAIN count : DINT;
                NON_RETAIN scratch : DINT;
                total : DINT;
            END_STRUCT;
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR RETAIN kept : DINT; END_VAR
        VAR lost : DINT; END_VAR
            kept := kept + 1;
            lost := lost + 1;
        END_FUNCTION_BLOCK

        PROGRAM P
            g.count := g.count + 1; g.scratch := g.scratch + 1; g.total := g.total + 1;
            h.count := h.count + 1; h.scratch := h.scratch + 1; h.total := h.total + 1;
            k.a := k.a + 1; k.b := k.b + 1;
            fbi();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            g : Counter;
            fbi : Fb;
            k : STRUCT RETAIN a : DINT; b : DINT; END_STRUCT;
        END_VAR
        VAR_GLOBAL RETAIN
            h : Counter;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![
            ("fbi.kept".to_string(), 4),
            ("g.count".to_string(), 4),
            ("h.count".to_string(), 4),
            ("h.total".to_string(), 4),
            ("k.a".to_string(), 4),
        ]
    );

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    let after = [
        ("g.count", 4),
        ("g.scratch", 1),
        ("g.total", 1),
        ("h.count", 4),
        ("h.scratch", 1),
        ("h.total", 4),
        ("fbi.kept", 4),
        ("fbi.lost", 1),
        ("k.a", 4),
        ("k.b", 1),
    ];
    for (path, value) in after {
        assert_eq!(read(&warm, path), VarValue::I32(value), "{path}");
    }
}

/// A member a base declares `RETAIN`, and a marked element in a base's
/// struct, are kept through `EXTENDS`: the instance is walked as it is laid
/// out, bases first, as a program member and as a global.
#[rstest]
fn an_inherited_member_is_kept(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Base
        VAR RETAIN x : DINT; END_VAR
        VAR c : Counter; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Child EXTENDS Base
        VAR y : DINT; END_VAR
            x := x + 1; c.count := c.count + 1; c.total := c.total + 1; y := y + 1;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR ch : Child; END_VAR
            ch(); gch();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL gch : Child; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![
            ("gch.c.count".to_string(), 4),
            ("gch.x".to_string(), 4),
            ("P1.ch.c.count".to_string(), 4),
            ("P1.ch.x".to_string(), 4),
        ]
    );

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    for inst in ["P1.ch", "gch"] {
        for (member, value) in [("x", 4), ("c.count", 4), ("c.total", 1), ("y", 1)] {
            let path = format!("{inst}.{member}");
            assert_eq!(read(&warm, &path), VarValue::I32(value), "{path}");
        }
    }
}

/// A constant is never kept, whatever its struct marks: a warm start after
/// a build that changed it leaves it at the new build's value.
#[rstest]
fn a_constant_takes_the_value_of_the_build(mut with_db: db::RootDatabase) {
    let program = |count: i32| {
        format!(
            r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR CONSTANT k : Counter := (count := {count}); END_VAR
        VAR c : Counter; END_VAR
            c.count := c.count + k.count + gk.count;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT gk : Counter := (count := {count}); END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#
        )
    };
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, &program(1));
    assert_eq!(ranges(&mir), vec![("P1.c.count".to_string(), 4)]);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");

    // The next build changes both constants; the saved ranges go into it.
    let mut next_db = db::RootDatabase::default();
    let (next_mir, next_wasm) = compile_to_mir_and_wasm(&mut next_db, &program(2));
    assert_eq!(ranges(&next_mir), ranges(&mir));
    let mut warm = TestPlc::load(&next_wasm).expect("reload");
    for (saved, range) in mir
        .retain_map
        .ranges
        .iter()
        .zip(&next_mir.retain_map.ranges)
    {
        let bytes = plc
            .read_bytes(saved.addr, saved.size as usize)
            .expect("snapshot");
        warm.write_bytes(range.addr, &bytes).expect("restore");
    }
    warm.run(1).expect("scan");

    let info = DebugInfo::from_wasm(&next_wasm);
    let read = |path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&warm.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    assert_eq!(read("P1.k.count"), VarValue::I32(2));
    assert_eq!(read("gk.count"), VarValue::I32(2));
    assert_eq!(read("P1.c.count"), VarValue::I32(6 + 4));
}

/// A constant alone, in the program or in a block it holds, leaves the
/// program out of the retain band.
#[rstest]
fn a_constant_alone_keeps_its_program_out_of_the_band(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK K
        VAR CONSTANT k : Counter := (count := 1); END_VAR
        VAR_OUTPUT o : DINT; END_VAR
            o := k.count;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR CONSTANT k : Counter := (count := 1); END_VAR
        VAR fk : K; x : DINT; END_VAR
            fk(); x := k.count + fk.o;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.retain_size, 0);
    assert!(ranges(&mir).is_empty());
}

/// The three rows a variable of a marked struct can take. `VAR` keeps the
/// marked elements, `VAR RETAIN` every element but the `NON_RETAIN` ones,
/// and `VAR NON_RETAIN` nothing, whatever the elements say.
#[rstest]
fn a_variable_keeps_by_its_own_row(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Counter : STRUCT
                RETAIN count : DINT;
                NON_RETAIN scratch : DINT;
                name : STRING[8];
            END_STRUCT;
            Pair : STRUCT
                a : Counter;
                RETAIN b : Counter;
                NON_RETAIN c : Counter;
                RETAIN rows : ARRAY[0..1] OF Counter;
            END_STRUCT;
        END_TYPE

        PROGRAM P
        VAR p : Pair; END_VAR
        VAR RETAIN r : Pair; END_VAR
        VAR NON_RETAIN n : Pair; END_VAR
            p.a.count := 1; r.a.count := 1; n.a.count := 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let paths: Vec<&str> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "P1.p.a.count",
            "P1.p.b.count",
            "P1.p.b.name",
            "P1.p.rows[0].count",
            "P1.p.rows[0].name",
            "P1.p.rows[1].count",
            "P1.p.rows[1].name",
            "P1.r.a.count",
            "P1.r.a.name",
            "P1.r.b.count",
            "P1.r.b.name",
            "P1.r.rows[0].count",
            "P1.r.rows[0].name",
            "P1.r.rows[1].count",
            "P1.r.rows[1].name",
        ]
    );
}

/// A `VAR RETAIN` holder reaches into the blocks it holds: a nested
/// instance keeps everything but its `NON_RETAIN` members, a `NON_RETAIN`
/// instance at any level keeps nothing, and a plain holder keeps what the
/// nested block marks `RETAIN` and nothing else.
#[rstest]
fn a_retained_instance_reaches_into_the_blocks_it_holds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR RETAIN kept : DINT; END_VAR
        VAR NON_RETAIN lost : DINT; END_VAR
        VAR plain : DINT; END_VAR
            kept := kept + 1; lost := lost + 1; plain := plain + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Wrapper
        VAR inner : Inner; END_VAR
        VAR NON_RETAIN ni : Inner; END_VAR
            inner(); ni();
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN w : Wrapper; END_VAR
        VAR NON_RETAIN nw : Wrapper; END_VAR
        VAR w2 : Wrapper; END_VAR
            w(); nw(); w2();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![
            ("P1.w.inner.kept".to_string(), 4),
            ("P1.w.inner.plain".to_string(), 4),
            ("P1.w2.inner.kept".to_string(), 4),
        ]
    );

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    let after = [
        ("P1.w.inner.kept", 4),
        ("P1.w.inner.plain", 4),
        ("P1.w.inner.lost", 1),
        ("P1.w.ni.kept", 1),
        ("P1.nw.inner.kept", 1),
        ("P1.w2.inner.kept", 4),
        ("P1.w2.inner.plain", 1),
    ];
    for (path, value) in after {
        assert_eq!(read(&warm, path), VarValue::I32(value), "{path}");
    }
}

/// What points at a marked struct without holding it gets no range: a
/// `VAR_IN_OUT`, a `REF_TO`, a `VAR_EXTERNAL`, a `VAR_TEMP`, and a FUNCTION's
/// locals and the block it holds. Only the global's own storage is listed.
#[rstest]
fn what_points_at_a_marked_struct_keeps_nothing(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Fb
        VAR RETAIN kept : DINT; END_VAR
            kept := kept + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Use
        VAR_IN_OUT c : Counter; END_VAR
            c.total := c.total + 1;
        END_FUNCTION_BLOCK

        FUNCTION f : DINT
        VAR c : Counter; fb : Fb; END_VAR
            fb(); f := c.count;
        END_FUNCTION

        PROGRAM P
        VAR u : Use; p : REF_TO Counter; x : DINT; END_VAR
        VAR_EXTERNAL g : Counter; END_VAR
        VAR_TEMP t : Counter; END_VAR
            u(c := g); x := f(); p := REF(g); t.count := 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : Counter; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(ranges(&mir), vec![("g.count".to_string(), 4)]);
}

/// A `STRING` element comes back whole: its range is its whole storage,
/// and a value filled to capacity survives a warm start.
#[rstest]
fn a_string_element_comes_back_whole(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; name : STRING[8]; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN r : Counter; END_VAR
            IF r.count = 0 THEN r.name := 'ABCDEFGH'; END_IF;
            r.count := r.count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let name = info.resolve("P1.r.name").expect("name");
    let range = mir
        .retain_map
        .ranges
        .iter()
        .find(|r| r.path == "P1.r.name")
        .expect("a range for the name");
    assert_eq!(range.size, name.size, "the range covers the whole STRING");

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(2).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    let read = |path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&warm.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    // The name is written on the first scan only: after the warm start it
    // can only have come back with the count.
    assert_eq!(read("P1.r.count"), VarValue::I32(3));
    assert_eq!(read("P1.r.name"), VarValue::String("ABCDEFGH".into()));
}

/// A struct with no qualifier in it is one range as before, inside a
/// retained member and as a retained global, and a struct that says
/// nothing in a `VAR` stays out of the map.
#[rstest]
fn an_unqualified_struct_is_one_range_as_before(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR p : Pt; END_VAR
        VAR RETAIN r : Pt; END_VAR
            r.x := r.x + p.x;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL RETAIN g : Pt; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![("g".to_string(), 8), ("P1.r".to_string(), 8)]
    );
}

/// A marked element alone puts its program in the retain band: nothing in
/// the program says `RETAIN` but the struct.
#[rstest]
fn a_marked_element_alone_puts_its_program_in_the_band(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR c : Counter; END_VAR
            c.count := c.count + 1; c.total := c.total + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert!(mir.retain_size > 0, "the instance is in the band");
    assert_eq!(ranges(&mir), vec![("P1.c.count".to_string(), 4)]);

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    assert_eq!(read(&warm, "P1.c.count"), VarValue::I32(4));
    assert_eq!(read(&warm, "P1.c.total"), VarValue::I32(1));
}

/// `PROGRAM NON_RETAIN` at the configuration keeps nothing of the
/// instance, a marked element included, and leaves it out of the band; a
/// plain instance of the same program beside it keeps what its type marks.
#[rstest]
fn a_non_retain_instance_keeps_nothing(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; total : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR c : Counter; END_VAR
        VAR RETAIN r : DINT; END_VAR
            c.count := c.count + 1; r := r + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
                PROGRAM NON_RETAIN P2 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![("P1.c.count".to_string(), 4), ("P1.r".to_string(), 4)]
    );
    let band = mir.retain_base..mir.retain_base + mir.retain_size;
    for inst in mir
        .schedule
        .as_ref()
        .expect("a schedule")
        .tasks
        .iter()
        .flat_map(|t| &t.programs)
    {
        let name = inst.inst_name_with_case.text(&with_db);
        assert_eq!(band.contains(&inst.instance_addr), name == "P1", "{name}");
    }

    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    let mut warm = warm_start(&mir, &wasm, &plc);
    warm.run(1).expect("scan");
    for (path, value) in [
        ("P1.c.count", 4),
        ("P1.r", 4),
        ("P2.c.count", 1),
        ("P2.r", 1),
    ] {
        assert_eq!(read(&warm, path), VarValue::I32(value), "{path}");
    }
}

/// Pinned, not promised: a `RETAIN` element inside a `VAR RETAIN` splits
/// the struct into one range per element, where the same struct with no
/// qualifier is one range. The follow-up that lists every leaf and strides
/// arrays flips this assertion instead of changing maps silently.
#[rstest]
fn a_redundant_qualifier_splits_the_range_today(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN
            r : STRUCT RETAIN x : DINT; y : DINT; END_STRUCT;
        END_VAR
            r.x := r.x + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        ranges(&mir),
        vec![("P1.r.x".to_string(), 4), ("P1.r.y".to_string(), 4)]
    );
}

/// A timer held by a retained instance starts over after a warm start: its
/// step and clock readings are `NON_RETAIN` in the standard library, so a
/// TON that was running when the snapshot was taken is not measured from a
/// reading of the previous run's clock, which started over too.
#[rstest]
fn a_timer_in_a_retained_instance_starts_over(mut with_db: db::RootDatabase) {
    use crate::tests::codegen::compile_to_mir_and_wasm_with_libraries;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    let source = r#"
        USING Std.Timers;

        FUNCTION_BLOCK Machine
        VAR t : TON; END_VAR
        VAR_OUTPUT q : BOOL; END_VAR
            t(IN := TRUE, PT := T#1s);
            q := t.Q;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN m : Machine; END_VAR
            m();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Timers.st"),
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        source,
    );
    let paths: Vec<&str> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| r.path.as_str())
        .collect();
    assert!(
        paths.contains(&"P1.m.t.IN"),
        "the timer's inputs are kept: {paths:?}"
    );
    assert!(
        !paths
            .iter()
            .any(|p| p.ends_with(".STATE") || p.ends_with("_TIME") || p.ends_with(".PREV_IN")),
        "its step and clock readings are not: {paths:?}"
    );

    // A PLC on a clock the test moves by hand, in milliseconds.
    let boot = |now_ms: i64| -> (TestPlc, Arc<AtomicI64>) {
        let clock = Arc::new(AtomicI64::new(now_ms * 1_000_000));
        let moved = Arc::clone(&clock);
        let plc = TestPlc::load_with_host(&wasm, move |linker| {
            linker.func_wrap("wasi:clocks/monotonic-clock@0.2.6", "now", move || {
                moved.load(Ordering::Relaxed)
            })?;
            Ok(())
        })
        .expect("load");
        (plc, clock)
    };
    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let elapsed_ms = |plc: &TestPlc| match read(plc, "P1.m.t.ET") {
        VarValue::I32(ms) => i64::from(ms),
        VarValue::U32(ms) => i64::from(ms),
        other => panic!("ET decoded as {other:?}"),
    };

    // The timer starts at 500 ms and has run 300 ms when the snapshot is taken.
    let (mut plc, clock) = boot(500);
    plc.scan().expect("the timer starts");
    clock.store(800 * 1_000_000, Ordering::Relaxed);
    plc.scan().expect("running");
    assert_eq!(elapsed_ms(&plc), 300);
    assert_eq!(read(&plc, "P1.m.q"), VarValue::Bool(false));
    let saved: Vec<Vec<u8>> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| plc.read_bytes(r.addr, r.size as usize).expect("snapshot"))
        .collect();

    // A power cycle: the clock starts over, the ranges come back.
    let (mut warm, clock) = boot(100);
    for (range, bytes) in mir.retain_map.ranges.iter().zip(&saved) {
        warm.write_bytes(range.addr, bytes).expect("restore");
    }
    warm.scan().expect("the first scan after the warm start");
    assert_eq!(read(&warm, "P1.m.q"), VarValue::Bool(false));
    clock.store(1000 * 1_000_000, Ordering::Relaxed);
    warm.scan().expect("900 ms after the warm start");
    assert_eq!(
        elapsed_ms(&warm),
        900,
        "measured from the warm start, not from the previous run's reading"
    );
    assert_eq!(read(&warm, "P1.m.q"), VarValue::Bool(false));
    clock.store(1100 * 1_000_000, Ordering::Relaxed);
    warm.scan().expect("one second after the warm start");
    assert_eq!(read(&warm, "P1.m.q"), VarValue::Bool(true));
}

/// A warm start resets a timer's step but brings its outputs back: a
/// pulse that was running comes back with `Q` `TRUE`, an off-delay with
/// its `ET`. The first call sets both from the step again, so the pulse
/// is over and the delay at zero, and the next rising edge starts them
/// as a fresh timer would.
#[rstest]
fn a_pulse_and_an_off_delay_in_a_retained_instance_start_over(mut with_db: db::RootDatabase) {
    use crate::tests::codegen::compile_to_mir_and_wasm_with_libraries;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    let source = r#"
        USING Std.Timers;

        FUNCTION_BLOCK Machine
        VAR_INPUT run : BOOL; END_VAR
        VAR p : TP; f : TOF; END_VAR
        VAR_OUTPUT pq, fq : BOOL; END_VAR
            p(IN := run, PT := T#1s);
            f(IN := run, PT := T#1s);
            pq := p.Q;
            fq := f.Q;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR run : BOOL; END_VAR
        VAR RETAIN m : Machine; END_VAR
            m(run := run);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Timers.st"),
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        source,
    );

    // A PLC on a clock the test moves by hand, in milliseconds.
    let boot = |now_ms: i64| -> (TestPlc, Arc<AtomicI64>) {
        let clock = Arc::new(AtomicI64::new(now_ms * 1_000_000));
        let moved = Arc::clone(&clock);
        let plc = TestPlc::load_with_host(&wasm, move |linker| {
            linker.func_wrap("wasi:clocks/monotonic-clock@0.2.6", "now", move || {
                moved.load(Ordering::Relaxed)
            })?;
            Ok(())
        })
        .expect("load");
        (plc, clock)
    };
    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc, path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    let elapsed_ms = |plc: &TestPlc, path: &str| match read(plc, path) {
        VarValue::I32(ms) => i64::from(ms),
        VarValue::U32(ms) => i64::from(ms),
        other => panic!("{path} decoded as {other:?}"),
    };
    let run = |plc: &mut TestPlc, on: bool| {
        let loc = info.resolve("P1.run").expect("P1.run");
        plc.write_bytes(loc.address, &[u8::from(on)]).expect("run");
    };

    // The pulse starts at 500 ms; `run` drops at 800 ms, which starts the
    // off-delay; the snapshot is taken at 1000 ms, with both running.
    let (mut plc, clock) = boot(500);
    run(&mut plc, true);
    plc.scan().expect("the pulse starts");
    assert_eq!(read(&plc, "P1.m.pq"), VarValue::Bool(true));
    clock.store(800 * 1_000_000, Ordering::Relaxed);
    run(&mut plc, false);
    plc.scan().expect("the off-delay starts");
    clock.store(1000 * 1_000_000, Ordering::Relaxed);
    plc.scan().expect("both running");
    assert_eq!(read(&plc, "P1.m.pq"), VarValue::Bool(true));
    assert_eq!(elapsed_ms(&plc, "P1.m.p.ET"), 500);
    assert_eq!(read(&plc, "P1.m.fq"), VarValue::Bool(true));
    assert_eq!(elapsed_ms(&plc, "P1.m.f.ET"), 200);
    let saved: Vec<Vec<u8>> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| plc.read_bytes(r.addr, r.size as usize).expect("snapshot"))
        .collect();

    // A power cycle with `run` low: the first scan ends the pulse and the
    // delay, since the step they came from is gone.
    let (mut warm, clock) = boot(100);
    for (range, bytes) in mir.retain_map.ranges.iter().zip(&saved) {
        warm.write_bytes(range.addr, bytes).expect("restore");
    }
    warm.scan().expect("the first scan after the warm start");
    assert_eq!(
        read(&warm, "P1.m.pq"),
        VarValue::Bool(false),
        "the pulse does not outlive the step that started it"
    );
    assert_eq!(elapsed_ms(&warm, "P1.m.p.ET"), 0);
    assert_eq!(read(&warm, "P1.m.fq"), VarValue::Bool(false));
    assert_eq!(elapsed_ms(&warm, "P1.m.f.ET"), 0);

    // A rising edge after the warm start starts a full pulse.
    run(&mut warm, true);
    warm.scan().expect("a new pulse");
    assert_eq!(read(&warm, "P1.m.pq"), VarValue::Bool(true));
    assert_eq!(elapsed_ms(&warm, "P1.m.p.ET"), 0);
    assert_eq!(read(&warm, "P1.m.fq"), VarValue::Bool(true));
    clock.store(1200 * 1_000_000, Ordering::Relaxed);
    warm.scan().expect("1100 ms after the new pulse");
    assert_eq!(read(&warm, "P1.m.pq"), VarValue::Bool(false));
    assert_eq!(elapsed_ms(&warm, "P1.m.p.ET"), 1000);
}

/// A scalar VAR_IN_OUT of a block is a member: the retain map lists it by
/// its section, `RETAIN` kept and `NON_RETAIN` not, and a warm start brings
/// the kept one back into the instance and the other to its start value:
/// three scans leave `count` 6 and `step` 4, and the scan after the warm
/// start gives 7, not 10.
#[rstest]
fn an_in_out_member_follows_its_section(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_IN_OUT RETAIN count : INT; END_VAR
        VAR_IN_OUT NON_RETAIN step : INT := 1; END_VAR
            count := count + step;
            step := step + 1;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR c : Counter; n : INT; END_VAR
            c();
            n := c.count;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(ranges(&mir), vec![("P1.c.count".to_string(), 4)]);

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("three scans");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&info, "P1.n"), Some(VarValue::I16(6)));
    assert_eq!(plc.read_var(&info, "P1.c.step"), Some(VarValue::I16(4)));

    let mut warm = warm_start(&mir, &wasm, &plc);
    assert_eq!(warm.read_var(&info, "P1.c.count"), Some(VarValue::I16(6)));
    assert_eq!(warm.read_var(&info, "P1.c.step"), Some(VarValue::I16(1)));
    warm.run(1).expect("a scan after the warm start");
    assert_eq!(warm.read_var(&info, "P1.n"), Some(VarValue::I16(7)));
}
