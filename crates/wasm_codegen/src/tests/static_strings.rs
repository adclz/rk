//! Static STRING storage: STRING as PROGRAM/FB instance fields and config
//! globals (read/write/copy), driven through the runtime. These exercise the
//! unified string place-addressing (header address via the same machinery as
//! scalars), not just function-local strings.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::{Config, Plc};

/// Decode the string at the start of the retain band: `[len:i32]` + bytes.
fn read_retain_string(plc: &Plc) -> String {
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    String::from_utf8_lossy(&r[4..4 + len]).to_string()
}

/// Assigning a literal to a STRING instance field, then reading it back.
#[rstest]
fn string_field_assign_literal(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            s := 'hello';
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
    assert_eq!(read_retain_string(&plc), "hello");
}

/// Copying one STRING instance field into another (`dst := src`).
#[rstest]
fn string_field_to_field_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN dst : STRING; END_VAR
        VAR src : STRING; END_VAR
            src := 'world';
            dst := src;
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
    assert_eq!(read_retain_string(&plc), "world");
}

/// A STRING instance-field initializer is applied at load (`__init`), before
/// any scan.
#[rstest]
fn string_field_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING := 'init!'; END_VAR
            ; // no-op body; the initializer is what we check
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let plc = Plc::load(&wasm, Config::default()).expect("load");
    assert_eq!(read_retain_string(&plc), "init!", "initializer applied at load");
}

/// A STRING VAR_GLOBAL initializer is applied at load and visible to programs.
#[rstest]
fn string_global_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Mirror
        VAR RETAIN seen : STRING; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING := 'globinit'; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "globinit");
}

/// A struct initializer with a STRING field (aggregate + string init together).
#[rstest]
fn struct_with_string_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Person : STRUCT name : STRING; age : INT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN p : Person := (name := 'bob', age := 30); END_VAR
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

    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let r = plc.read_retain();
    // p.name (STRING) at offset 0; p.age (INT) at offset 4+80 = 84.
    assert_eq!(read_retain_string(&plc), "bob");
    assert_eq!(i32::from_le_bytes(r[84..88].try_into().unwrap()), 30);
}

/// Assigning a STRING-returning call's result into an instance field
/// (producer result → field via rk.str_assign).
#[rstest]
fn string_call_result_into_field(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION echo : STRING
        VAR_INPUT x : STRING; END_VAR
            echo := x;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            s := echo('hi there');
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
    assert_eq!(read_retain_string(&plc), "hi there");
}

/// Passing a STRING instance field as a by-value VAR_INPUT argument.
#[rstest]
fn string_field_as_argument(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION echo : STRING
        VAR_INPUT x : STRING; END_VAR
            echo := x;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN out : STRING; END_VAR
        VAR src : STRING; END_VAR
            src := 'fieldarg';
            out := echo(src);
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
    assert_eq!(read_retain_string(&plc), "fieldarg");
}

/// Passing a STRING instance field as a `VAR_IN_OUT` argument (by reference):
/// the callee mutates the field in place. Exercises the (header_addr, cap)
/// flattening for a field place — previously only `StringMemory` locals worked.
#[rstest]
fn string_field_as_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set_hi
        VAR_IN_OUT s : STRING; END_VAR
            s := 'hi-inout';
        END_FUNCTION

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            set_hi(s);
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
    assert_eq!(read_retain_string(&plc), "hi-inout");
}

/// A declared `STRING[N]` capacity is honored for an instance FIELD: assigning a
/// longer string clamps to N (with the old bug, fields defaulted to capacity 80
/// and would store the whole string).
#[rstest]
fn sized_string_field_clamps_to_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING[3]; END_VAR
            s := 'hello';
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
    assert_eq!(read_retain_string(&plc), "hel", "STRING[3] clamps 'hello'");
}

/// `STRING[N]` capacity is honored for a VAR_GLOBAL too.
#[rstest]
fn sized_string_global_clamps_to_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN seen : STRING[10]; END_VAR
            g := 'abcdef';
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING[4]; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "abcd", "STRING[4] global clamps 'abcdef'");
}

/// A STRING VAR_GLOBAL written by one program and read by another.
#[rstest]
fn string_global_shared(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Setter
            g := 'shared';
        END_PROGRAM

        PROGRAM Mirror
        VAR RETAIN seen : STRING; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Setter;
                PROGRAM P2 WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(2).expect("scans"); // Setter writes g, Mirror copies it into seen
    assert_eq!(read_retain_string(&plc), "shared");
}
