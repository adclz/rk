//! Tests for constant variable initializers: program statics, globals, and the
//! cold-vs-warm-start interaction with RETAIN. Run once at load via `__init`.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::{Config, Plc};

fn read_first_i32(plc: &Plc) -> i32 {
    i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap())
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rk_init_{tag}_{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// A program field initializer runs once at load (before any scan), then the
/// scan logic proceeds from it.
#[rstest]
fn program_field_initializer_runs_once(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN x : INT := 7; END_VAR
            x := x + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    assert_eq!(read_first_i32(&plc), 7, "initializer applied at load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 8, "then incremented by the scan");
}

/// A constant-expression initializer (arithmetic over literals) is evaluated
/// and applied at load.
#[rstest]
fn const_expr_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN x : INT := 2 + 3 * 4; END_VAR
            x := x + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    assert_eq!(read_first_i32(&plc), 14, "2 + 3*4 = 14 at load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 15);
}

/// A config VAR_GLOBAL initializer is applied at load and visible to programs.
#[rstest]
fn global_initializer_applied(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Mirror
        VAR RETAIN seen : INT; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : INT := 42; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan"); // Mirror copies g (== 42) into seen
    let seen = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    assert_eq!(seen, 42, "global initialized to 42, read by the program");
}

/// A 1-D array initializer `[10, 20, 30]` is applied element-by-element at load.
#[rstest]
fn array_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN seen : INT; END_VAR
        VAR a : ARRAY[0..2] OF INT := [10, 20, 30]; END_VAR
            seen := a[0] + a[1] + a[2];
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 60, "10 + 20 + 30 from the array init");
}

/// A struct initializer `(x := 3, y := 4)` is applied field-by-field at load.
#[rstest]
fn struct_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN seen : INT; END_VAR
        VAR pt : Point := (x := 3, y := 4); END_VAR
            seen := pt.x * 100 + pt.y;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 304, "x*100 + y = 3*100 + 4 from struct init");
}

/// A RETAIN var's initializer is its COLD-start value only: on a warm restart
/// the persisted value overrides the initializer (`__init` runs, then restore).
#[rstest]
fn retain_initializer_is_cold_start_only(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN x : INT := 7; END_VAR
            x := x + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let path = temp_path("coldwarm");

    // Cold boot: x initializes to 7, two scans -> 9, snapshot.
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (cold)");
        assert_eq!(read_first_i32(&plc), 7);
        plc.run(2).expect("scans");
        assert_eq!(read_first_i32(&plc), 9);
        plc.snapshot_retain().expect("snapshot");
    }

    // Warm boot: `__init` sets x = 7, but restore overrides it back to 9.
    {
        let plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (warm)");
        assert_eq!(
            read_first_i32(&plc),
            9,
            "warm start: persisted value overrides the initializer"
        );
    }

    std::fs::remove_file(&path).ok();
}
