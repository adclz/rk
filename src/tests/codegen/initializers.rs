//! Tests for constant variable initializers: program statics, globals, and the
//! cold-vs-warm-start interaction with RETAIN. Run once at load via `__init`.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
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

/// Nested multi-dim bracket init `[[1,2,3],[4,5,6]]` fills row-major. (Was a
/// silent runtime zero before the HIR-authoritative resolution refactor — MIR
/// dropped nested brackets.)
#[rstest]
fn nested_multidim_array_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Matrix : ARRAY[0..1, 0..2] OF INT; END_TYPE

        PROGRAM P
        VAR RETAIN m : Matrix := [[1, 2, 3], [4, 5, 6]]; END_VAR
            ;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // `m` is the only RETAIN field → the band IS the flattened matrix. Read the
    // 6 INT elements (row-major) directly. (Multi-dim element ACCESS in a body is
    // a separate, unimplemented feature, so we verify the init via the band.)
    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let r = plc.read_retain();
    let vals: Vec<i32> = (0..6)
        .map(|i| i32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap()))
        .collect();
    assert_eq!(
        vals,
        vec![1, 2, 3, 4, 5, 6],
        "row-major flatten of [[1,2,3],[4,5,6]]"
    );
}

/// Multi-dimensional element ACCESS via chained brackets `m[i][j]` (the valid IEC
/// syntax — the comma form `m[i,j]` is initializer-only). Each chained `Index`
/// addresses one dimension; the positional checksum pins every cell row-major.
/// (`DINT` because the weights overflow 16-bit `INT`.)
#[rstest]
fn multidim_element_access(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Matrix : ARRAY[0..1, 0..2] OF DINT; END_TYPE

        PROGRAM P
        VAR RETAIN total : DINT; END_VAR
        VAR m : Matrix := [[1, 2, 3], [4, 5, 6]]; END_VAR
            total := m[0][0]*100000 + m[0][1]*10000 + m[0][2]*1000
                   + m[1][0]*100 + m[1][1]*10 + m[1][2];
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
    assert_eq!(read_first_i32(&plc), 123456, "m[i][j] reads row-major");
}

/// 3-D access `c[i][j][k]` — verifies `index_dimension` counts arbitrary chain
/// depth and the stride product generalizes past 2-D (2×2×2, flat 1..8).
#[rstest]
fn three_dim_element_access(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN total : DINT; END_VAR
        VAR c : ARRAY[0..1, 0..1, 0..1] OF DINT
              := [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]; END_VAR
            total := c[0][0][0]*10000000 + c[0][0][1]*1000000
                   + c[0][1][0]*100000   + c[0][1][1]*10000
                   + c[1][0][0]*1000     + c[1][0][1]*100
                   + c[1][1][0]*10       + c[1][1][1];
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
    assert_eq!(read_first_i32(&plc), 12345678, "c[i][j][k] reads row-major");
}

/// Writing `m[i][j] := v` uses the same per-dimension offset as reads: overwrite
/// one cell of an initialized matrix and confirm only that cell changed. `m[1][0]`
/// (weight ×100) goes 4→9, so the checksum shifts by +500 (123456 → 123956); a
/// wrong write offset would change a different weight.
#[rstest]
fn multidim_element_write(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN total : DINT; END_VAR
        VAR m : ARRAY[0..1, 0..2] OF DINT := [[1, 2, 3], [4, 5, 6]]; END_VAR
            m[1][0] := 9;
            total := m[0][0]*100000 + m[0][1]*10000 + m[0][2]*1000
                   + m[1][0]*100 + m[1][1]*10 + m[1][2];
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
    assert_eq!(
        read_first_i32(&plc),
        123956,
        "m[1][0]:=9 hits only flat index 3"
    );
}

/// Nested repetition `[2(3(5))]` = 2×(3×5) = six 5s. (Was a silent runtime zero.)
#[rstest]
fn nested_repetition_array_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN total : INT; END_VAR
        VAR a : ARRAY[0..5] OF INT := [2(3(5))]; END_VAR
            total := a[0] + a[1] + a[2] + a[3] + a[4] + a[5];
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
    assert_eq!(read_first_i32(&plc), 30, "[2(3(5))] = six 5s");
}

/// Underscore-separated repeat count `[1_0(7)]` = ten 7s. (Was a silent runtime
/// zero — MIR's old `parse::<u32>()` choked on the `_`.)
#[rstest]
fn underscore_repeat_count_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN total : INT; END_VAR
        VAR a : ARRAY[0..9] OF INT := [1_0(7)]; END_VAR
            total := a[0]+a[1]+a[2]+a[3]+a[4]+a[5]+a[6]+a[7]+a[8]+a[9];
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
    assert_eq!(read_first_i32(&plc), 70, "[1_0(7)] = ten 7s");
}

/// A FUNCTION-local aggregate initializer is applied (prepended, re-run each
/// call). Previously dropped — `lower_var_init` only handled scalar locals.
#[rstest]
fn function_local_array_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_init : INT
        VAR a : ARRAY[0..2] OF INT := [100, 20, 3]; END_VAR
            sum_init := a[0] + a[1] + a[2];
        END_FUNCTION

        PROGRAM P
        VAR RETAIN seen : INT; END_VAR
            seen := sum_init();
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
    assert_eq!(
        read_first_i32(&plc),
        123,
        "function-local array init [100,20,3]"
    );
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
    assert_eq!(
        read_first_i32(&plc),
        304,
        "x*100 + y = 3*100 + 4 from struct init"
    );
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
                retain_path: Some(path.clone()),
                program_path: None,
                ..Config::default()
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
                retain_path: Some(path.clone()),
                program_path: None,
                ..Config::default()
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

/// Regression: a PROGRAM field or VAR_GLOBAL whose type is not i32-shaped had
/// its `__init` store emitted as `i32.store` regardless of the value's width,
/// so the module failed wasm validation as a whole — reported against
/// `__init`, with nothing pointing back at the initializer that caused it.
/// `rk compile` still exited 0 and wrote the unloadable artifact out.
///
/// `place_type` (wasm_codegen/src/emit_stmt.rs) had no `MirPlace::Global` arm
/// and fell through to an `Int` default; `Global` is the variant `__init`
/// stores through, so *every* REAL/LREAL/LINT/LWORD/LTIME initializer on a
/// program field or global was affected.
#[rstest]
fn wide_and_float_program_field_initializers_load(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN
            ok : DINT;
        END_VAR
        VAR
            r : REAL := 1.5;
            d : LREAL := 2.25;
            l : LINT := 5000000000;
            w : LWORD := 16#1122334455667788;
        END_VAR
            IF r = 1.5 AND d = 2.25 AND l = 5000000000 AND w = 16#1122334455667788 THEN
                ok := 1;
            ELSE
                ok := 0;
            END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    // Loading is the assertion that used to fail: the module did not validate.
    let mut plc = Plc::load(&wasm, Config::default()).expect("module must validate and load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 1, "every wide initializer applied");
}

/// Same defect reached through a config-level `VAR_GLOBAL`, which is the other
/// producer of `MirPlace::Global` stores in `__init`.
#[rstest]
fn wide_and_float_global_initializers_load(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN
            ok : DINT;
        END_VAR
        VAR_EXTERNAL
            gr : REAL;
            gl : LINT;
        END_VAR
            IF gr = 3.5 AND gl = 9000000000 THEN ok := 1; ELSE ok := 0; END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                gr : REAL := 3.5;
                gl : LINT := 9000000000;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("module must validate and load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 1, "wide global initializers applied");
}

/// A CONFIGURATION's `VAR_GLOBAL` initializer runs before the first scan, and
/// the value is visible through `VAR_EXTERNAL` to a PROGRAM instantiated inside
/// a RESOURCE — globals are application-scoped, so the grouping is irrelevant.
#[rstest]
fn config_global_initializer_applies(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN
            seen : DINT;
        END_VAR
        VAR_EXTERNAL
            g : DINT;
        END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                g : DINT := 42;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_first_i32(&plc), 42, "the config global's initializer ran");
}

/// Every name in a global list is its own storage, and the shared
/// initializer reaches all of them. Before the builder split the list, the
/// declaration produced ONE global named after the whole spec text.
#[rstest]
fn a_global_name_list_gives_each_name_its_own_slot(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL
            ga : DINT;
            gb : DINT;
        END_VAR
        VAR RETAIN seen : DINT; END_VAR
            ga := ga + 1;
            seen := ga * 100 + gb;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                ga, gb : DINT := 5;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    // Both start at 5; ga is bumped to 6 and gb is untouched, so the two
    // names cannot be aliasing one slot.
    assert_eq!(read_first_i32(&plc), 605, "ga=6 and gb=5 are separate storage");
}
