//! Static STRING storage: STRING as PROGRAM/FB instance fields and config
//! globals (read/write/copy), driven through the runtime. These exercise the
//! unified string place-addressing (header address via the same machinery as
//! scalars), not just function-local strings.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
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
    assert_eq!(
        read_retain_string(&plc),
        "init!",
        "initializer applied at load"
    );
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
    assert_eq!(
        read_retain_string(&plc),
        "abcd",
        "STRING[4] global clamps 'abcdef'"
    );
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

/// Regression: `ARRAY[..] OF STRING[n]` used to lay out 4+80-byte elements and
/// never truncate, because `lower_array_type` lowered the element through
/// `lower_type` alone. `Type::normalize` collapses `STRING[n]` and plain
/// `STRING` onto the same type, so the declared length only survives on the
/// SPEC — and this was the one call site that did not consult it.
///
/// Asserted through TRUNCATION, not through a neighbouring guard: the wrong
/// layout over-allocates (84 bytes per element instead of 8), so nothing is
/// ever clobbered and a guard variable passes either way. What actually
/// differs is how much of the source string the element keeps.
#[rstest]
fn array_of_sized_strings_truncates_at_the_declared_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[0..1] OF STRING[4];
        END_VAR
            a[0] := 'ABCDEFGHIJKLMNOP';
            IF a[0] = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "an element of ARRAY OF STRING[4] holds 4 characters");
}

/// A `STRING[n]` reached through a `TYPE` alias keeps its length. The alias
/// carries a `Target` spec, so the `SizedString` sits on the data type's own
/// spec one hop away; not following that hop silently gave every aliased
/// string the 80-byte default.
#[rstest]
fn aliased_sized_string_truncates_at_the_declared_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : STRING[4]; END_TYPE

        FUNCTION run : DINT
        VAR
            s : Small;
        END_VAR
            s := 'ABCDEFGHIJKLMNOP';
            IF s = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "an aliased STRING[4] holds 4 characters");
}

/// A declared `STRING[n]` keeps its length in EVERY container it can appear in.
///
/// The length is not part of type identity — `STRING[4] := STRING[80]` is legal
/// and truncates, and two lengths must not read as different overloads — so it
/// survives only on the spec, and every container has to lower through the
/// spec-aware path. Three separate bugs came from one container forgetting:
/// an `ARRAY OF STRING[4]` with 84-byte elements, a `TYPE` alias silently
/// widened to 80, and a struct field overrunning its slot.
///
/// One test over all of them, so a container that regresses is visible next to
/// the ones that do not.
#[rstest]
#[case::direct("s", "VAR s : STRING[4]; END_VAR")]
#[case::alias("s", "VAR s : Small; END_VAR")]
#[case::struct_field("r.f", "VAR r : Rec; END_VAR")]
#[case::fb_member("h.s", "VAR h : Holder; END_VAR")]
#[case::array_element("a[1]", "VAR a : ARRAY[0..1] OF STRING[4]; END_VAR")]
#[case::array_of_alias("b[1]", "VAR b : ARRAY[0..1] OF Small; END_VAR")]
#[case::struct_in_array("c[1].f", "VAR c : ARRAY[0..1] OF Rec; END_VAR")]
fn a_sized_string_keeps_its_length_in_every_container(
    mut with_db: db::RootDatabase,
    #[case] target: &str,
    #[case] decl: &str,
) {
    let source = format!(
        r#"
        TYPE Small : STRING[4]; END_TYPE
        TYPE Rec : STRUCT f : STRING[4]; g : DINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Holder
        VAR
            s : STRING[4];
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        {decl}
            {target} := 'ABCDEFGHIJKLMNOP';
            IF {target} = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#
    );
    let wasm = super::compile_to_wasm(&mut with_db, &source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "`{target}` must hold exactly its declared 4 characters");
}

/// Escape sequences decode to the bytes they DENOTE, not the source text.
/// MIR used to intern the raw literal bytes while HIR validated the decoded
/// form, so `'A$0AB'` was checked as 3 characters and executed as 5 — and a
/// program's strings silently carried `$`-signs into production. One decoder
/// (`parse_single_byte_string`) now serves both.
#[rstest]
fn string_escapes_decode_to_denoted_bytes(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            (* $41='A', $$ = one dollar, $N = LF, $T = tab, $'= quote *)
            s := '$41$$$N$T$'';
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
    assert_eq!(read_retain_string(&plc), "A$\n\t'");
}
