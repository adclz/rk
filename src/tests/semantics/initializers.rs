use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

// E0401: a once-per-type initializer (TYPE default, FB/CLASS member default,
// static PROGRAM field or config global) must be constant. CONSTANT
// references fold; anything site-dependent is refused. FUNCTION and METHOD
// locals are exempt — they re-initialize per call and are not type members.

#[rstest]
fn a_global_init_from_a_non_constant_is_refused(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL
    a : DINT := 5;
    b : DINT := a;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:5:17 ]
       |
     5 |     b : DINT := a;
       |                 |
       |                 `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'a' is an ordinary variable; declare it CONSTANT if its value never changes
    ---'
    ");
}

#[rstest]
fn an_fb_member_default_from_a_global_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb
    VAR m : DINT := some_global; END_VAR
END_FUNCTION_BLOCK

CONFIGURATION Cfg
VAR_GLOBAL some_global : DINT := 5; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:3:21 ]
       |
     3 |     VAR m : DINT := some_global; END_VAR
       |                     ^^^^^|^^^^^
       |                          `------- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'some_global' is an ordinary variable; declare it CONSTANT if its value never changes
    ---'
    ");
}

#[rstest]
fn a_type_default_from_a_config_constant_is_refused(mut with_db: RootDatabase) {
    // A TYPE has no view into a CONFIGURATION's scope; the reference cannot
    // fold once-per-type. Declaring the constant where the scope chain sees
    // it (VAR_EXTERNAL CONSTANT in an FB) is the spelling that folds.
    let source = r#"
TYPE AliasK : INT := K; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT K : INT := 7; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:2:22 ]
       |
     2 | TYPE AliasK : INT := K; END_TYPE
       |                      |
       |                      `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'K' IS a CONSTANT, but a TYPE declaration cannot see it: a TYPE default folds only literals, arithmetic, and constants in its own scope
    ---'
    ");
}

#[rstest]
fn a_function_local_keeps_its_runtime_initializer(mut with_db: RootDatabase) {
    // The carve-out: FUNCTION locals are not type members and re-initialize
    // per call, so a runtime-evaluated init stays legal.
    let source = r#"
FUNCTION f : DINT
    VAR x : DINT := g2; END_VAR
    f := x;
END_FUNCTION

CONFIGURATION Cfg
VAR_GLOBAL g2 : DINT := 9; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn a_constant_cycle_is_refused_not_looped(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL CONSTANT
    k1 : DINT := k2;
    k2 : DINT := k1;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:4:18 ]
       |
     4 |     k1 : DINT := k2;
       |                  ^|
       |                   `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'k2' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ---'
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:5:18 ]
       |
     5 |     k2 : DINT := k1;
       |                  ^|
       |                   `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'k1' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ---'
    ");
}

#[rstest]
fn a_global_init_from_a_visible_constant_is_accepted(mut with_db: RootDatabase) {
    // The shape users hit most: sizing a global from a named CONSTANT in the
    // same scope. It FOLDS — no diagnostic. (The value side is pinned by
    // codegen::initializers::global_init_from_constant_folds.)
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL CONSTANT k : DINT := 7; END_VAR
VAR_GLOBAL g : DINT := k; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn an_uninitialized_constant_is_refused_with_its_own_reason(mut with_db: RootDatabase) {
    // Not a cycle and not a non-constant initializer: there is simply no
    // value. The note must say so rather than reach for the catch-all.
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL CONSTANT k : DINT; END_VAR
VAR_GLOBAL g : DINT := k; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:4:24 ]
       |
     4 | VAR_GLOBAL g : DINT := k; END_VAR
       |                        |
       |                        `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'k' is CONSTANT but declares no initial value, so there is nothing to fold
    ---'
    ");
}

#[rstest]
fn a_method_local_keeps_its_runtime_initializer(mut with_db: RootDatabase) {
    // The other half of the header's claim: METHOD locals are exempt like
    // FUNCTION locals — per-call, not type members. The FB MEMBER next to it
    // stays under the rule.
    let source = r#"
FUNCTION_BLOCK fb
    VAR base : DINT := 2; END_VAR
    METHOD scaled : DINT
        VAR x : DINT := g2; END_VAR
        scaled := x + base;
    END_METHOD
END_FUNCTION_BLOCK

CONFIGURATION Cfg
VAR_GLOBAL g2 : DINT := 9; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Dummy;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Dummy
    VAR t : INT; END_VAR
    t := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An array copy moves bytes, so the element type must be the SAME: an
// `ARRAY OF INT` into an `ARRAY OF REAL` used to check clean at every door and
// read back garbage (b[1] was not 2.0). Literals still convert one by one.

#[rstest]
fn an_array_does_not_widen_its_elements(mut with_db: RootDatabase) {
    let source = r#"
TYPE Row : ARRAY[0..2] OF INT; END_TYPE
TYPE RowD : ARRAY[0..2] OF DINT; END_TYPE

FUNCTION g : REAL
    VAR_INPUT arr : ARRAY[0..2] OF REAL; END_VAR
    g := arr[1];
END_FUNCTION

FUNCTION f : REAL
    VAR
        a : ARRAY[0..2] OF INT := [1, 2, 3];
        b : ARRAY[0..2] OF REAL := a;
        s : ARRAY[0..2] OF SINT;
        n : ARRAY[0..1] OF Row; d : ARRAY[0..1] OF RowD;
    END_VAR
    b := a;
    a := s;
    d := n;
    f := g(arr := a);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:33 ]
        |
     13 |         b : ARRAY[0..2] OF REAL := a;
        |                                 ^^|^
        |                                   `--- expected 'ARRAY [0..2] OF REAL', got 'ARRAY [0..2] OF INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:17:10 ]
        |
     13 |         b : ARRAY[0..2] OF REAL := a;
        |         |
        |         `-- type is declared by variable 'b' here
        |
     17 |     b := a;
        |          |
        |          `-- expected 'ARRAY [0..2] OF REAL', got 'ARRAY [0..2] OF INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:18:10 ]
        |
     12 |         a : ARRAY[0..2] OF INT := [1, 2, 3];
        |         |
        |         `-- type is declared by variable 'a' here
        |
     18 |     a := s;
        |          |
        |          `-- expected 'ARRAY [0..2] OF INT', got 'ARRAY [0..2] OF SINT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:19:10 ]
        |
     15 |         n : ARRAY[0..1] OF Row; d : ARRAY[0..1] OF RowD;
        |                                 |
        |                                 `-- type is declared by variable 'd' here
        |
     19 |     d := n;
        |          |
        |          `-- expected 'ARRAY [0..1] OF RowD', got 'ARRAY [0..1] OF Row'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:19 ]
        |
      6 |     VAR_INPUT arr : ARRAY[0..2] OF REAL; END_VAR
        |               ^|^
        |                `--- type is declared by variable 'arr' here
        |
     20 |     f := g(arr := a);
        |                   |
        |                   `-- expected 'ARRAY [0..2] OF REAL', got 'ARRAY [0..2] OF INT'
    ----'
    ");
}

#[rstest]
fn an_array_initializer_converts_literals_and_a_same_type_copy_is_accepted(
    mut with_db: RootDatabase,
) {
    let source = r#"
TYPE A3 : ARRAY[0..2] OF INT; END_TYPE

FUNCTION f : REAL
    VAR
        r : ARRAY[0..2] OF REAL := [1, 2, 3];
        a : A3 := [1, 2, 3];
        b : ARRAY[0..2] OF INT := a;
        c : A3;
        i : INT := 1;
    END_VAR
    b := a;
    c := b;
    r[0] := a[i];
    f := r[0] + r[1];
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A struct alias's override names a field the struct does not have: a
/// diagnostic, not a silently dropped store.
#[rstest]
fn invalid_struct_alias_default_names_an_unknown_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Point : STRUCT
                x : INT := 3;
                y : INT := 5;
            END_STRUCT;
            Origin : Point := (z := 7);
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: no such field
       ,-[ file:///test0.st:7:32 ]
       |
     7 |             Origin : Point := (z := 7);
       |                                ^^^|^^
       |                                   `---- 'Point' has no field named 'z'
       |
       | Note: 'Point' has fields with similar name:
       |       - x
       |       - y
    ---'
    ");
}

/// A STRING initializer has to fit the capacity, whether the source writes
/// one or takes the default. Reading only a written `STRING[N]` let both the
/// default and a length naming a constant through, and the codegen then
/// truncated the value with nothing said.
#[rstest]
fn invalid_string_initializer_over_capacity(mut with_db: RootDatabase) {
    let source = format!(
        r#"
TYPE Alias5 : STRING[5]; END_TYPE

FUNCTION_BLOCK fb
VAR CONSTANT
    SIZE : INT := 5;
END_VAR
VAR
    written : STRING[5] := 'far too long';
    named : STRING[SIZE] := 'far too long';
    alias : Alias5 := 'far too long';
    plain : STRING := '{}';
    fits : STRING := 'fine';
END_VAR
END_FUNCTION_BLOCK
"#,
        "z".repeat(100)
    );
    let rendered = test_diagnostics(&mut with_db, &[&source]);
    let over: Vec<&str> = rendered
        .lines()
        .filter_map(|line| line.split("STRING literal ").nth(1))
        .collect();

    assert_snapshot!(over.join("\n"), @r"
    exceeds the capacity of 5 bytes, got 12; declare it STRING[12], or shorten the literal
    exceeds the capacity of 5 bytes, got 12; declare it STRING[12], or shorten the literal
    exceeds the capacity of 5 bytes, got 12; declare it STRING[12], or shorten the literal
    exceeds the capacity of 80 bytes, got 100; declare it STRING[100], or shorten the literal
    ");
}
