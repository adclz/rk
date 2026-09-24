//! VAR_CONFIG values: an instance's own starting values, given in the
//! CONFIGURATION rather than in the POU, one instance at a time.

use crate::tests::codegen::TestPlc;
use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

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
