// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! VAR_CONFIG values: an instance's own starting values, given in the
//! CONFIGURATION rather than in the POU, one instance at a time.

use crate::tests::codegen::TestPlc;
use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// A resource may share its name with a program instance. `Main.x` is that
/// instance's variable, and `Main.Main.y` and `Main.Other.x` go through the
/// resource: each value lands on the variable it names, and no other.
#[rstest]
fn a_value_through_a_resource_named_like_an_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR x : INT := 1; y : INT := 2; END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Main.x       : INT := 10;
            Main.Main.y  : INT := 20;
            Main.Other.x : INT := 30;
        END_VAR
            RESOURCE Main ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Main WITH T : P;
                PROGRAM Other WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.read_var(&info, "Main.x"), Some(VarValue::I16(10)));
    assert_eq!(plc.read_var(&info, "Main.y"), Some(VarValue::I16(20)));
    assert_eq!(plc.read_var(&info, "Other.x"), Some(VarValue::I16(30)));
    assert_eq!(plc.read_var(&info, "Other.y"), Some(VarValue::I16(2)));
}

/// A value overrides the declaration's for that instance only, whether it
/// names a variable, a member of an instance the program holds, or the
/// instance as a whole. A value for the instance sets the members it names
/// and leaves the others at their declared values.
#[rstest]
fn a_value_starts_one_instance_at_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR PUBLIC speed : INT := 2; rpm : INT := 9; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR x : INT := 1; fb : Motor; END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Res.P1.x        : INT := 10;
            Res.P1.fb.speed : INT := 20;
            Res.P2.fb       : Motor := (speed := 30);
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
                PROGRAM P2 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let plc = TestPlc::load(&wasm).expect("load");
    let read = |path: &str| {
        let loc = info.resolve(path).expect(path);
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };
    assert_eq!(read("P1.x"), VarValue::I16(10));
    assert_eq!(read("P1.fb.speed"), VarValue::I16(20));
    assert_eq!(read("P2.x"), VarValue::I16(1), "P2 keeps the declaration's");
    assert_eq!(
        read("P2.fb.speed"),
        VarValue::I16(30),
        "set through the instance"
    );
    assert_eq!(
        read("P2.fb.rpm"),
        VarValue::I16(9),
        "not named, so declared"
    );
}

/// A value for a variable VAR_CONFIG locates is its channel's starting
/// value, from the entry that locates it or from one of its own.
#[rstest]
fn a_value_for_a_located_variable_starts_its_channel(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Drive
        VAR out AT %Q* : INT; level AT %M* : INT; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR d : Drive; END_VAR
            d();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Res.P1.d.out   AT %QW0 : INT := 5;
            Res.P1.d.level AT %MW2 : INT;
            Res.P1.d.level : INT := 7;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");
    let word = |a: &str| i16::from_le_bytes(plc.read_located(a).expect(a)[..2].try_into().unwrap());
    assert_eq!(word("%QW0"), 5);
    assert_eq!(word("%MW2"), 7);
}

/// A value for a RETAIN variable is a starting value like any other: a cold
/// start begins at it, and a warm start resumes where the last power cycle
/// left instead.
#[rstest]
fn a_value_for_a_retained_variable_is_the_cold_start(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN count : INT := 0; END_VAR
            count := count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Res.P1.count : INT := 100;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let read = |plc: &TestPlc| {
        let loc = info.resolve("P1.count").expect("P1.count");
        loc.decode(&plc.read_bytes(loc.address, loc.size as usize).unwrap())
    };

    let mut plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(read(&plc), VarValue::I16(100), "the VAR_CONFIG value");
    plc.run(3).expect("scans");
    assert_eq!(read(&plc), VarValue::I16(103));
    let saved: Vec<Vec<u8>> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| plc.read_bytes(r.addr, r.size as usize).expect("snapshot"))
        .collect();

    let mut cold = TestPlc::load(&wasm).expect("reload");
    cold.run(1).expect("scan");
    assert_eq!(
        read(&cold),
        VarValue::I16(101),
        "a cold start begins at the value"
    );

    let mut warm = TestPlc::load(&wasm).expect("reload");
    for (range, bytes) in mir.retain_map.ranges.iter().zip(&saved) {
        warm.write_bytes(range.addr, bytes).expect("restore");
    }
    warm.run(1).expect("scan");
    assert_eq!(read(&warm), VarValue::I16(104), "a warm start resumes");
}

/// A value can go to a field of a STRUCT: this instance's, over the defaults of the
/// type and over the variable's own initializer, which its other fields keep.
#[rstest]
fn a_value_starts_a_field_of_a_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Inner : STRUCT b : INT := 2; c : INT := 7; END_STRUCT;
            S : STRUCT
                a : INT := 1;
                keep : INT := 1;
                inner : Inner;
                whole : Inner;
                list : ARRAY[0..1] OF INT;
                text : STRING[8];
            END_STRUCT;
            Holder : STRUCT m : Fb; END_STRUCT;
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR k : INT := 1; l : INT := 1; s : S; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Prog
        VAR_INPUT i : S; END_VAR
        VAR_OUTPUT o : S; END_VAR
        VAR
            s : S;
            set : S := (a := 21, keep := 22);
            place : STRUCT x : INT := 1; y : INT := 1; END_STRUCT;
            f : Fb;
            h : Holder;
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM One WITH T : Prog;
                PROGRAM Two WITH T : Prog;
            END_RESOURCE
            VAR_CONFIG
                Res.One.s.a : INT := 3;
                Res.One.s.inner.b : INT := 4;
                Res.One.s.whole : Inner := (b := 5);
                Res.One.s.list : ARRAY[0..1] OF INT := [6, 7];
                Res.One.s.text : STRING[8] := 'ab';
                Res.One.set.a : INT := 23;
                Res.One.place.x : INT := 8;
                Res.One.f.s.a : INT := 9;
                Res.One.i.a : INT := 10;
                Res.One.o.a : INT := 11;
                Res.One.h.m : Fb := (k := 12);
            END_VAR
        END_CONFIGURATION
    "#;
    let (_, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let plc = TestPlc::load(&wasm).expect("load");
    for (name, expected) in [
        ("One.s.a", 3),
        ("One.s.keep", 1),
        ("One.s.inner.b", 4),
        ("One.s.inner.c", 7),
        ("One.s.whole.b", 5),
        ("One.s.whole.c", 7),
        ("One.s.list[0]", 6),
        ("One.s.list[1]", 7),
        ("One.set.a", 23),
        ("One.set.keep", 22),
        ("One.place.x", 8),
        ("One.place.y", 1),
        ("One.f.s.a", 9),
        ("One.f.s.keep", 1),
        ("One.i.a", 10),
        ("One.i.keep", 1),
        ("One.o.a", 11),
        ("One.h.m.k", 12),
        ("One.h.m.l", 1),
        ("Two.s.a", 1),
        ("Two.s.inner.b", 2),
        ("Two.set.a", 21),
        ("Two.place.x", 1),
        ("Two.f.s.a", 1),
        ("Two.h.m.k", 1),
    ] {
        assert_eq!(
            plc.read_var(&info, name),
            Some(VarValue::I16(expected)),
            "{name}"
        );
    }
    assert_eq!(
        plc.read_var(&info, "One.s.text"),
        Some(VarValue::String("ab".to_string()))
    );
    assert_eq!(
        plc.read_var(&info, "Two.s.text"),
        Some(VarValue::String(String::new()))
    );
}

/// A value for a field of a RETAIN variable is the cold start, as for the variable.
#[rstest]
fn a_value_for_a_retained_field_is_the_cold_start(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT count : INT; other : INT := 5; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN s : S; END_VAR
            s.count := s.count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Res.P1.s.count : INT := 100;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);

    let mut plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.read_var(&info, "P1.s.count"), Some(VarValue::I16(100)));
    assert_eq!(plc.read_var(&info, "P1.s.other"), Some(VarValue::I16(5)));
    plc.run(3).expect("scans");
    assert_eq!(plc.read_var(&info, "P1.s.count"), Some(VarValue::I16(103)));
    assert_eq!(mir.retain_map.ranges.len(), 1);
}
