//! End-to-end tests for VAR_GLOBAL: shared state across programs, accessed both
//! via VAR_EXTERNAL and directly by name (type-faithful).

use crate::tests::{compile_to_mir_and_wasm, with_db};
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
