// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! End-to-end tests for VAR_GLOBAL: shared state across programs, accessed both
//! via VAR_EXTERNAL and directly by name.

use crate::tests::codegen::TestPlc;
use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// Two programs share a config `VAR_GLOBAL`. `Inc` increments it (via
/// `VAR_EXTERNAL`); `Mirror` copies it into a retained `seen` (direct access,
/// no `VAR_EXTERNAL`). Both run each scan in declaration order, so `seen`
/// tracks the shared global — proving global read/write, sharing, and both
/// access styles work end-to-end.
#[rstest]
fn programs_share_a_global(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Inc
        VAR_EXTERNAL g : INT; END_VAR
            g := g + 1;
        END_PROGRAM

        PROGRAM Mirror
        VAR RETAIN seen : INT; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Inc;
                PROGRAM P2 WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Only Mirror's `seen` is retained (one INT), so the band reads it.
    assert_eq!(mir.retain_size, 2);

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");

    // Each scan: Inc does g := g+1, then Mirror does seen := g. After 3 scans
    // the shared global g == 3, so seen == 3.
    let seen = i16::from_le_bytes(plc.read_retain()[..2].try_into().unwrap());
    assert_eq!(seen, 3, "Mirror saw the shared global incremented by Inc");
}

/// The host (HMI) can read and write a config `VAR_GLOBAL` through the exposed
/// globals region: `__init` sets it, the host reads it, writes a new value, and
/// the next scan sees the host-written value.
#[rstest]
fn host_reads_and_writes_global(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN seen : DINT; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : DINT := 100; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");

    // __init set g = 100; the host sees it in the globals region.
    assert_eq!(plc.globals_region().size, 4, "one DINT global");
    assert_eq!(
        i32::from_le_bytes(plc.read_globals()[0..4].try_into().unwrap()),
        100
    );

    // The program copies g into `seen`.
    plc.run(1).expect("scan");
    assert_eq!(
        i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap()),
        100
    );

    // Host writes g = 777; the next scan sees it.
    plc.write_globals(0, &777i32.to_le_bytes())
        .expect("write global");
    plc.run(1).expect("scan");
    assert_eq!(
        i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap()),
        777,
        "program saw the host-written global"
    );
}

/// A RETAIN global is BOTH host-visible (globals region) AND persisted (retain
/// band): it sits in the overlap, so both regions start at its address.
#[rstest]
fn retain_global_overlaps_both_regions(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR x : DINT; END_VAR
            x := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL RETAIN g : DINT := 55; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");

    let gr = plc.globals_region();
    let rr = plc.retain_region();
    assert_eq!(gr.size, 4, "globals band = the one retain global");
    assert_eq!(rr.size, 4, "retain band = the one retain global");
    assert_eq!(gr.base, rr.base, "retain global is the overlap: same base");
    assert_eq!(
        i32::from_le_bytes(plc.read_globals()[0..4].try_into().unwrap()),
        55
    );
}

/// A CONFIGURATION's own names are its globals before they are types, as a
/// body's variables come before what its namespace names. Names are caseless,
/// and a CONSTANT `limit` beside a type `Limit` was E0317 wherever the
/// configuration named it: here it starts a global and a program's variable,
/// and is an array's last index.
#[rstest]
fn a_global_named_like_a_type_is_the_global(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Limit : INT; END_TYPE

        PROGRAM P
        VAR_EXTERNAL a : ARRAY[0..4] OF INT; END_VAR
        VAR x : INT; END_VAR
            a[4] := x + 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT limit : INT := 4; END_VAR
        VAR_GLOBAL g : INT := limit; a : ARRAY[0..limit] OF INT; END_VAR
        VAR_CONFIG Res.P1.x : INT := limit; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.read_var(&info, "g"), Some(VarValue::I16(4)));
    assert_eq!(plc.read_var(&info, "P1.x"), Some(VarValue::I16(4)));
    plc.scan().expect("scan");
    assert_eq!(plc.read_var(&info, "a[4]"), Some(VarValue::I16(5)));
}
