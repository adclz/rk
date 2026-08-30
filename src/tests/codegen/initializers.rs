//! Tests for constant variable initializers: program statics, globals, and the
//! cold-vs-warm-start interaction with RETAIN. Run once at load via `__init`.

use crate::tests::codegen::{compile_to_mir_and_wasm, compile_to_wasm, with_db};
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

/// Multi-dimensional element ACCESS via chained brackets `m[i][j]`. Both forms
/// are accepted since the conformance arc: `m[i, j]` is the standard's own
/// spelling (one subscript list — see
/// `multi_dim_initialization_fills_rightmost_fastest`), and the chained form
/// pins that each `Index` addresses one dimension. The positional checksum
/// pins every cell row-major. (`DINT` because the weights overflow 16-bit
/// `INT`.)
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
    assert_eq!(
        read_first_i32(&plc),
        42,
        "the config global's initializer ran"
    );
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
    assert_eq!(
        read_first_i32(&plc),
        605,
        "ga=6 and gb=5 are separate storage"
    );
}

/// The standard's repetition initializer: `8(-4095)` fills eight slots.
#[rstest]
fn repetition_initializer_fills_its_count(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[1..16] OF INT := [8(-4095), 8(4095)];
        END_VAR
            (* accumulate in the DINT lane: INT arithmetic wraps at 16 bits,
               by the pinned sub-width invariant *)
            run := a[8];
            run := run * 10000 + a[9];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(
        result,
        -4095 * 10000 + 4095,
        "slot 8 ends the first group, slot 9 begins the second"
    );
}

/// A repetition factor applies to a GROUP: `[2(1, 2, 3)]` is the sequence
/// 1, 2, 3, 1, 2, 3 — the standard's own example.
#[rstest]
fn repetition_initializer_repeats_a_group(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[0..5] OF INT := [2(1, 2, 3)];
        END_VAR
            run := a[0];
            run := run * 10 + a[1];
            run := run * 10 + a[2];
            run := run * 10 + a[3];
            run := run * 10 + a[4];
            run := run * 10 + a[5];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 123123, "1,2,3,1,2,3 read back positionally");
}

/// During initialization the RIGHTMOST subscript varies most rapidly: a
/// [0..1, 0..2] array filled from [1..6] puts 1,2,3 in row 0 and 4,5,6 in
/// row 1.
#[rstest]
fn multi_dim_initialization_fills_rightmost_fastest(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            m : ARRAY[0..1, 0..2] OF INT := [1, 2, 3, 4, 5, 6];
        END_VAR
            run := m[0, 2] * 100 + m[1, 0] * 10 + m[1, 2];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 346, "m[0,2]=3, m[1,0]=4, m[1,2]=6: row-major fill");
}

// ---------------------------------------------------------------------------
// Implicitly-WIDENING initializers: the declared type wins over the
// expression's. `r : REAL := 1 + 1` is INT arithmetic HIR accepts by
// widening; lowering it at the expression's lane made invalid wasm for
// locals and globals, and for memory-resident fields stored integer BITS
// into the REAL slot (2 read back as 2.8e-45) — all at exit 0.
// ---------------------------------------------------------------------------

/// Every widening pair on a FUNCTION local, pinned by VALUE.
#[rstest]
fn widening_local_initializers_carry_the_value(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fr : REAL
        VAR r : REAL := 1 + 1; END_VAR
            fr := r;
        END_FUNCTION

        FUNCTION fl : LREAL
        VAR r : LREAL := 1 + 1; END_VAR
            fl := r;
        END_FUNCTION

        FUNCTION fi : LINT
        VAR r : LINT := 1 + 1; END_VAR
            fi := r;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: f32 = crate::tests::codegen::execute_wasm(&wasm, "fr", ());
    assert_eq!(r, 2.0);
    let l: f64 = crate::tests::codegen::execute_wasm(&wasm, "fl", ());
    assert_eq!(l, 2.0);
    let i: i64 = crate::tests::codegen::execute_wasm(&wasm, "fi", ());
    assert_eq!(i, 2);
}

/// The memory-store half of the same bug: an FB member default and a
/// VAR_INPUT default, read back through the instance.
#[rstest]
fn widening_member_initializers_carry_the_value(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR_INPUT gain : REAL := 1 + 1; END_VAR
        VAR bias : REAL := 2 + 3; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION use : REAL
        VAR i : fb; END_VAR
            i();
            use := i.gain + i.bias;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let v: f32 = crate::tests::codegen::execute_wasm(&wasm, "use", ());
    assert_eq!(v, 7.0, "2.0 + 5.0, not integer bits reinterpreted");
}

/// The OTHER trigger of the member-init bug, no widening involved: a plain
/// literal on an FB's or CLASS's first REAL member, instantiated as a
/// FUNCTION local. The old `Local` shortcut addressed the leaf as the
/// INSTANCE local, whose memory info stores i32 — `f32.const` under
/// `i32.store`, invalid wasm at exit 0. Only the `whole` flag fixes this
/// one; the declared-lane cast never fires (from == to).
#[rstest]
fn literal_member_initializer_on_a_local_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR r : REAL := 2.5; END_VAR
        END_FUNCTION_BLOCK

        CLASS c
        VAR r : REAL := 1.25; END_VAR
            METHOD get : REAL
                get := r;
            END_METHOD
        END_CLASS

        FUNCTION use : REAL
        VAR
            i : fb;
            o : c;
        END_VAR
            i();
            use := i.r + o.get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let v: f32 = crate::tests::codegen::execute_wasm(&wasm, "use", ());
    assert_eq!(v, 3.75, "2.5 + 1.25 through both instance kinds");
}

/// The static half: a config global's widening initializer, applied by
/// `__init` at load.
#[rstest]
fn widening_global_initializer_carries_the_value(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL g : REAL; END_VAR
            g := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : REAL := 1 + 1; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let g = f32::from_le_bytes(plc.read_globals()[..4].try_into().unwrap());
    assert_eq!(g, 2.0);
}

/// Cold/warm across a MULTI-WORD retain band: `__init` fills a RETAIN array,
/// a scan mutates several elements, and the warm restore must bring back the
/// WHOLE band — a restore that only rewrote the first word passes the scalar
/// cold/warm test above but fails the far element here.
#[rstest]
fn retain_array_restores_the_whole_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN a : ARRAY[0..3] OF DINT := [10, 20, 30, 40]; END_VAR
            a[0] := a[0] + 1;
            a[3] := a[3] + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let path = temp_path("retain_array");

    let read4 = |plc: &Plc| -> Vec<i32> {
        let r = plc.read_retain();
        (0..4)
            .map(|i| i32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap()))
            .collect()
    };

    // Cold: init fills the band, two scans bump the ends twice.
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
        assert_eq!(read4(&plc), vec![10, 20, 30, 40], "initializer fills all");
        plc.run(2).expect("scans");
        assert_eq!(read4(&plc), vec![12, 20, 30, 42]);
        plc.snapshot_retain().expect("snapshot");
    }

    // Warm: __init re-fills [10,20,30,40], then restore must overwrite ALL of
    // it — first word AND last.
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
            read4(&plc),
            vec![12, 20, 30, 42],
            "the whole band restored, not just the first word"
        );
    }

    std::fs::remove_file(&path).ok();
}

// ---------------------------------------------------------------------------
// KNOWN BUG, pinned until the fold-vs-refuse decision: a STATIC initializer
// (config global, PROGRAM field) that references ANY variable — another
// global, or even a CONSTANT — checks clean and is silently DROPPED by
// `__init`: the slot stays 0. `lower_init_leaves`'s Static arm skips every
// non-`is_const_value` leaf, and nothing in HIR refuses it first.
//
// These tests assert TODAY's wrong behavior on purpose, so the fix cannot
// land without flipping them into real assertions. The RULING is decided
// (once-per-type): TYPE defaults, FB/CLASS member defaults and static-host
// initializers must be constant-foldable — CONSTANT references FOLD, and
// anything site-dependent (`:= SomeGlobal`) is REFUSED with a diagnostic,
// making the per-host divergence inexpressible. Plain FUNCTION locals keep
// runtime-evaluated inits; they are not type members.
// ---------------------------------------------------------------------------

#[rstest]
fn known_bug_global_init_from_global_is_silently_dropped(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL a : DINT; b : DINT; END_VAR
            a := a + 0;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            a : DINT := 5;
            b : DINT := a;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let g = plc.read_globals();
    let a = i32::from_le_bytes(g[..4].try_into().unwrap());
    let b = i32::from_le_bytes(g[4..8].try_into().unwrap());
    assert_eq!(a, 5);
    // WRONG on purpose: `b := a` should either yield 5 (declaration-ordered
    // stores) or be refused at check. When this assertion fails, the bug is
    // fixed — replace it with the decided behavior.
    assert_eq!(
        b, 0,
        "b is no longer zero — the silent drop is fixed: make this a real \
         assertion (b == 5 if stores are declaration-ordered, or delete the \
         test if `:= a` is now refused at check)"
    );
}

#[rstest]
fn known_bug_global_init_from_constant_is_silently_dropped(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL g : DINT; END_VAR
            g := g + 0;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT k : DINT := 7; END_VAR
        VAR_GLOBAL g : DINT := k; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let g = plc.read_globals();
    let k = i32::from_le_bytes(g[..4].try_into().unwrap());
    let v = i32::from_le_bytes(g[4..8].try_into().unwrap());
    assert_eq!(k, 7);
    // WRONG on purpose — `:= k` is textbook ST and must become 7 once
    // constant references fold. See project_static_init_dropped.
    assert_eq!(
        v, 0,
        "g is no longer zero — CONSTANT references now fold: assert v == 7 \
         and retire this pin"
    );
}

// ---------------------------------------------------------------------------
// TYPE-level defaults: `TYPE ... END_TYPE` initial values — alias `:= 5`
// ---------------------------------------------------------------------------

const TYPE_DEFAULTS: &str = r#"
    TYPE AliasInt : INT := 5; END_TYPE
    TYPE ChainInt : AliasInt; END_TYPE
    TYPE Pt : STRUCT x : INT := 3; y : INT := 4; END_STRUCT; END_TYPE
    TYPE Box : STRUCT origin : Pt; label : INT := 7; END_STRUCT; END_TYPE
    TYPE Row : ARRAY[0..1] OF Pt; END_TYPE
"#;

#[rstest]
fn type_defaults_apply_to_function_locals(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{TYPE_DEFAULTS}
        FUNCTION run : DINT
        VAR
            v : AliasInt;
            c : ChainInt;
            p : Pt;
            b : Box;
            r : Row;
        END_VAR
            (* accumulate in the DINT lane: INT wraps at 16 bits by the
               pinned sub-width invariant, and 55343 does not fit *)
            run := v;
            run := run * 10 + c;
            run := run * 10 + p.x;
            run := run * 10 + b.origin.y;
            run := run * 10 + r[1].x;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let v: i32 = crate::tests::codegen::execute_wasm(&wasm, "run", ());
    assert_eq!(v, 55343, "alias, chain, struct, nested, array-of-struct");
}

#[rstest]
fn a_partial_declaration_init_overlays_the_type_defaults(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{TYPE_DEFAULTS}
        FUNCTION run : INT
        VAR p : Pt := (y := 9); END_VAR
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let v: i32 = crate::tests::codegen::execute_wasm(&wasm, "run", ());
    assert_eq!(v, 309, "x keeps the TYPE's 3, y takes the declaration's 9");
}

#[rstest]
fn type_defaults_apply_to_fb_members(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{TYPE_DEFAULTS}
        FUNCTION_BLOCK holder
        VAR p : Pt; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR h : holder; END_VAR
            h();
            run := h.p.x * 100 + h.p.y;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let v: i32 = crate::tests::codegen::execute_wasm(&wasm, "run", ());
    assert_eq!(v, 304, "an FB member of a defaulted struct type");
}

#[rstest]
fn type_defaults_apply_to_the_static_hosts(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{TYPE_DEFAULTS}
        PROGRAM P
        VAR RETAIN seen : INT; END_VAR
        VAR p : Pt; END_VAR
        VAR_EXTERNAL g : Pt; END_VAR
            seen := p.x * 1000 + p.y * 100 + g.x * 10 + g.y;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : Pt; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, &source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(
        read_first_i32(&plc),
        3434,
        "TYPE defaults in a PROGRAM field AND a config global"
    );
}

/// Constant ARITHMETIC in a type default folds and applies — the control for
/// the known-bug pair below.
#[rstest]
fn type_default_with_const_arithmetic_applies(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE ArithInt : INT := 2 + 3; END_TYPE
        FUNCTION run : INT
        VAR v : ArithInt; END_VAR
            run := v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let v: i32 = crate::tests::codegen::execute_wasm(&wasm, "run", ());
    assert_eq!(v, 5);
}

/// KNOWN BUG, fifth surface of project_static_init_dropped: a type default
/// referencing a CONSTANT checks clean and reads ZERO — at BOTH hosts,
/// through two different silent mechanisms. Statics: the `is_const_value`
/// gate skips the leaf. Locals: the leaf's name does not resolve from the
/// host's scope, and codegen's variable-not-in-local-map arm quietly pushes
/// a constant 0 (that arm cannot be hardened to a panic until constant
/// folding lands, because this path reaches it).
#[rstest]
fn known_bug_type_default_constant_ref_is_zero_at_both_hosts(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE AliasK : INT := K; END_TYPE

        FUNCTION local_host : INT
        VAR v : AliasK; END_VAR
            local_host := v;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN seen : INT; END_VAR
        VAR s : AliasK; END_VAR
            seen := s * 10 + local_host();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT K : INT := 7; END_VAR
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
        0,
        "no longer zero — constant references now fold somewhere: 77 = both \
         hosts fixed (assert 77 and retire this pin), 70 = static host only, \
         7 = local host only"
    );
}
