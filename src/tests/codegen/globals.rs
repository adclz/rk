//! End-to-end tests for VAR_GLOBAL: shared state across programs, accessed both
//! via VAR_EXTERNAL and directly by name.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::{Config, Plc};

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
    assert_eq!(mir.retain_size, 4);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(3).expect("scans");

    // Each scan: Inc does g := g+1, then Mirror does seen := g. After 3 scans
    // the shared global g == 3, so seen == 3.
    let seen = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
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
    let mut plc = Plc::load(&wasm, Config::default()).expect("load");

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
    let plc = Plc::load(&wasm, Config::default()).expect("load");

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
