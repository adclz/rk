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
    // `g2` is reached without a VAR_EXTERNAL, which rk allows and the linter
    // warns about.
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
    // `g2` is reached without a VAR_EXTERNAL, which rk allows and the linter
    // warns about.
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
    exceeds the capacity of 5 bytes, got 12; change 'Alias5' to STRING[12] or use another type, or shorten the literal
    exceeds the capacity of 80 bytes, got 100; declare it STRING[100], or shorten the literal
    ");
}

/// A VAR_IN_OUT is bound to its argument by each call, so an instance's
/// initializer cannot give it a value. The value used to be written into
/// the pointer the binding lives in, and `__init` failed to validate.
#[rstest]
fn invalid_in_out_named_in_an_instance_initializer(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR_IN_OUT io : INT; END_VAR
VAR k : INT; END_VAR
    k := io;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
VAR inner : Drive := (io := 5); END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive := (io := 30, k := 2); o : Outer; END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0405] Error: member cannot be initialized
       ,-[ file:///test0.st:9:23 ]
       |
     3 | VAR_IN_OUT io : INT; END_VAR
       |            ^|
       |             `-- 'io' is declared here
       |
     9 | VAR inner : Drive := (io := 5); END_VAR
       |                       ^^^|^^^
       |                          `----- 'io' is a VAR_IN_OUT, which each call binds to its argument, so an initializer cannot give it a value
       |
       | Note: pass the variable in the call instead, as 'io := x'
    ---'
    [E0405] Error: member cannot be initialized
        ,-[ file:///test0.st:13:19 ]
        |
      3 | VAR_IN_OUT io : INT; END_VAR
        |            ^|
        |             `-- 'io' is declared here
        |
     13 | VAR d : Drive := (io := 30, k := 2); o : Outer; END_VAR
        |                   ^^^^|^^^
        |                       `----- 'io' is a VAR_IN_OUT, which each call binds to its argument, so an initializer cannot give it a value
        |
        | Note: pass the variable in the call instead, as 'io := x'
    ----'
    ");
}

/// No member without a value of its own can be named by an instance's
/// initializer (E0405), and the error points at where the member is
/// declared. A VAR_TEMP is made afresh by each call and a VAR_EXTERNAL names a
/// VAR_GLOBAL, so their value was silently dropped; a CONSTANT's reads fold
/// to its declared value while `__init` wrote the new one.
#[rstest]
#[case::in_out(
    "VAR_IN_OUT m : INT; END_VAR",
    "",
    "a VAR_IN_OUT, which each call binds to its argument"
)]
#[case::temp(
    "VAR_TEMP m : INT; END_VAR",
    "",
    "a VAR_TEMP, which each call makes afresh"
)]
#[case::external(
    "VAR_EXTERNAL m : INT; END_VAR",
    "VAR_GLOBAL m : INT; END_VAR",
    "a VAR_EXTERNAL, which names a VAR_GLOBAL"
)]
#[case::constant(
    "VAR CONSTANT m : INT := 1; END_VAR",
    "",
    "CONSTANT, whose value is its declaration's"
)]
fn invalid_member_an_instance_initializer_cannot_set(
    mut with_db: RootDatabase,
    #[case] decl: &str,
    #[case] global: &str,
    #[case] what: &str,
) {
    let source = format!(
        r#"
FUNCTION_BLOCK Drive
{decl}
VAR k : INT; END_VAR
    k := m;
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive := (m := 30, k := 2); END_VAR
END_PROGRAM

CONFIGURATION Cfg
{global}
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let rendered = test_diagnostics(&mut with_db, &[&source]);
    assert_eq!(
        rendered.matches("[E").count(),
        1,
        "only the member is refused, got:\n{rendered}"
    );
    assert!(
        rendered.contains("[E0405] Error: member cannot be initialized")
            && rendered.contains(&format!(
                "'m' is {what}, so an initializer cannot give it a value"
            ))
            && rendered.contains("'m' is declared here"),
        "`(m := 30)` on {decl} must be E0405 pointing at the declaration, got:\n{rendered}"
    );
}

/// An input's default is what the caller passes when the argument is
/// omitted, before the callee's own variables exist: a constant, which the
/// call site folds.
#[rstest]
fn an_input_default_is_a_constant(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green, Blue); END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT K : INT := 7; END_VAR
VAR_GLOBAL G : INT; END_VAR
END_CONFIGURATION

FUNCTION f : INT
VAR_INPUT
    a : INT := 1;
    b : INT := L * 2;
    c : INT := K;
    d : Color := Color#Blue;
    e : REF_TO INT := REF(G);
END_VAR
VAR CONSTANT L : INT := 3; END_VAR
VAR_EXTERNAL CONSTANT K : INT; END_VAR
VAR_EXTERNAL G : INT; END_VAR
    f := a + b + c;
END_FUNCTION

FUNCTION g : INT
    g := f();
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn an_input_default_from_a_variable_is_refused(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL G : INT; END_VAR
END_CONFIGURATION

FUNCTION one : INT
    one := 1;
END_FUNCTION

FUNCTION f : INT
VAR_INPUT
    a : INT := G;
    b : INT := a;
    c : INT := one();
    d : REF_TO INT := REF(a);
END_VAR
VAR_EXTERNAL G : INT; END_VAR
    f := a + b + c;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:12:16 ]
        |
     12 |     a : INT := G;
        |                |
        |                `-- this initial value must be a constant: the caller passes it
        |
        | Note: 'G' is an ordinary variable; declare it CONSTANT if its value never changes
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:13:16 ]
        |
     13 |     b : INT := a;
        |                |
        |                `-- this initial value must be a constant: the caller passes it
        |
        | Note: 'a' is another input of this call: it has no value before the call binds it
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:14:16 ]
        |
     14 |     c : INT := one();
        |                ^^|^^
        |                  `---- this initial value must be a constant: the caller passes it
        |
        | Note: a call is not a constant
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:15:23 ]
        |
     15 |     d : REF_TO INT := REF(a);
        |                       ^^^|^^
        |                          `---- this initial value must be a constant: the caller passes it
        |
        | Note: 'a' is another input of this call: it has no value before the call binds it
    ----'
    ");
}

/// A default computed from a sibling input or from the callee's own local is
/// refused too: neither exists when the caller fills in the argument, and the
/// caller's `a` and `y` are someone else's.
#[rstest]
fn an_input_default_from_a_sibling_or_a_local_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR_INPUT
    a : INT;
    b : INT := a * 2;
    c : INT := y;
END_VAR
VAR y : INT := 4; END_VAR
    f := b + c;
END_FUNCTION

FUNCTION caller : INT
VAR a : INT := 100; y : INT := 100; END_VAR
    caller := f(a := 3);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:5:16 ]
       |
     5 |     b : INT := a * 2;
       |                ^^|^^
       |                  `---- this initial value must be a constant: the caller passes it
       |
       | Note: 'a' is another input of this call: it has no value before the call binds it
    ---'
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:6:16 ]
       |
     6 |     c : INT := y;
       |                |
       |                `-- this initial value must be a constant: the caller passes it
       |
       | Note: 'y' belongs to the call, which has not started when the caller passes the default
    ---'
    ");
}

/// A `REF()` default the caller cannot compute before the call is refused,
/// written out or as a CONSTANT's value: a subscript naming another input,
/// the callee's own local, the instance's member.
#[rstest]
fn an_input_default_reference_the_caller_cannot_compute_is_refused(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL G : ARRAY[1..3] OF INT; END_VAR
END_CONFIGURATION

FUNCTION by_index : INT
VAR_INPUT
    i : INT;
    p : PInt := REF(G[i]);
END_VAR
VAR_EXTERNAL G : ARRAY[1..3] OF INT; END_VAR
    by_index := p^;
END_FUNCTION

FUNCTION by_constant : INT
VAR_INPUT p : PInt := PK; END_VAR
VAR loc : INT; END_VAR
VAR CONSTANT PK : PInt := REF(loc); END_VAR
    by_constant := p^;
END_FUNCTION

FUNCTION_BLOCK Motor
VAR speed : INT; END_VAR
VAR CONSTANT PS : PInt := REF(speed); END_VAR
    METHOD PUBLIC get : INT
    VAR_INPUT p : PInt := PS; END_VAR
        get := p^;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:11:17 ]
        |
     11 |     p : PInt := REF(G[i]);
        |                 ^^^^|^^^^
        |                     `------ this initial value must be a constant: the caller passes it
        |
        | Note: 'i' is another input of this call: it has no value before the call binds it
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:18:23 ]
        |
     18 | VAR_INPUT p : PInt := PK; END_VAR
        |                       ^|
        |                        `-- this initial value must be a constant: the caller passes it
        |
        | Note: 'loc' belongs to the call, which has not started when the caller passes the default
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:20:27 ]
        |
     20 | VAR CONSTANT PK : PInt := REF(loc); END_VAR
        |                           ^^^^|^^^
        |                               `----- a CONSTANT is one value for every instance and every call: a REF() in it names a global
        |
        | Note: 'loc' belongs to the call: each call has its own
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:26:27 ]
        |
     26 | VAR CONSTANT PS : PInt := REF(speed); END_VAR
        |                           ^^^^^|^^^^
        |                                `------ a CONSTANT is one value for every instance and every call: a REF() in it names a global
        |
        | Note: 'speed' is a member: each instance has its own
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:28:27 ]
        |
     28 |     VAR_INPUT p : PInt := PS; END_VAR
        |                           ^|
        |                            `-- this initial value must be a constant: the caller passes it
        |
        | Note: 'speed' is a member: each instance has its own
    ----'
    ");
}

/// In a TYPE's or a CONFIGURATION's initial value, a `REF()` names a field
/// beside it or a VAR_GLOBAL, through fields and subscripts, and is typed
/// like any other; a name that is nothing is E0201, where it read NULL.
#[rstest]
fn a_reference_in_a_type_or_configuration_default(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE
TYPE Inner : STRUCT v : INT := 3; END_STRUCT; END_TYPE
TYPE S : STRUCT
    a : INT;
    arr : ARRAY[0..2] OF INT;
    inn : Inner;
    pa : PInt := REF(a);
    parr : PInt := REF(arr[2]);
    pinn : PInt := REF(inn.v);
    bad : PInt := REF(nothing);
END_STRUCT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL
    g : INT;
    garr : ARRAY[0..1] OF INT;
    s : S;
    r : PInt := REF(g);
    rarr : PInt := REF(garr[1]);
    rs : PInt := REF(s.inn.v);
    greal : REAL;
    wrong : PInt := REF(greal);
END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0201] Error: no item found in scope
        ,-[ file:///test0.st:11:23 ]
        |
     11 |     bad : PInt := REF(nothing);
        |                       ^^^|^^^
        |                          `----- no item "nothing" found in scope
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:23:18 ]
        |
      2 | TYPE PInt : REF_TO INT; END_TYPE
        |             ^^^^^|^^^^
        |                  `------ type is defined by 'PInt' here
        |
     23 |     wrong : PInt := REF(greal);
        |                  ^^^^^^|^^^^^^
        |                        `-------- expected 'PInt', got 'REF_TO REAL'
    ----'
    "#);
}

/// A CONSTANT is one value for every instance and every call, so a `REF()`
/// in it is a global's address. `REF(speed)` was a different address in each
/// Motor, re-lowered wherever the CONSTANT was read, and `REF(loc)` in each
/// call.
#[rstest]
fn a_constant_reference_names_a_global(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL j : INT; END_VAR
VAR_GLOBAL CONSTANT KG : PInt := REF(j); END_VAR
END_CONFIGURATION

FUNCTION_BLOCK Motor
VAR speed : INT; END_VAR
VAR CONSTANT PS : PInt := REF(speed); END_VAR
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR loc : INT; END_VAR
VAR CONSTANT PK : PInt := REF(loc); END_VAR
VAR_EXTERNAL CONSTANT KG : PInt; END_VAR
    f := KG^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:11:27 ]
        |
     11 | VAR CONSTANT PS : PInt := REF(speed); END_VAR
        |                           ^^^^^|^^^^
        |                                `------ a CONSTANT is one value for every instance and every call: a REF() in it names a global
        |
        | Note: 'speed' is a member: each instance has its own
    ----'
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:16:27 ]
        |
     16 | VAR CONSTANT PK : PInt := REF(loc); END_VAR
        |                           ^^^^|^^^
        |                               `----- a CONSTANT is one value for every instance and every call: a REF() in it names a global
        |
        | Note: 'loc' belongs to the call: each call has its own
    ----'
    ");
}

/// REAL arithmetic over literals and CONSTANTs is a constant wherever one is
/// needed; over an ordinary variable it is not.
#[rstest]
fn real_arithmetic_is_a_constant(mut with_db: RootDatabase) {
    let source = r#"
TYPE R6 : REAL := 2.0 * 3.0; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT GK : LREAL := 1.0; END_VAR
VAR_GLOBAL third : LREAL := GK / 3.0; G : REAL; END_VAR
END_CONFIGURATION

FUNCTION_BLOCK B
VAR CONSTANT KR : REAL := 2.5; END_VAR
VAR r : REAL := -(KR + 0.5) * 2.0; bad : REAL := G * 2.0; END_VAR
VAR_EXTERNAL G : REAL; END_VAR
END_FUNCTION_BLOCK

FUNCTION f : REAL
VAR_INPUT x : REAL := KR / 4.0; END_VAR
VAR CONSTANT KR : REAL := 1.0; END_VAR
    f := x;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
        ,-[ file:///test0.st:11:50 ]
        |
     11 | VAR r : REAL := -(KR + 0.5) * 2.0; bad : REAL := G * 2.0; END_VAR
        |                                                  ^^^|^^^
        |                                                     `----- this initial value must be a constant: it is fixed before the program runs
        |
        | Note: 'G' is an ordinary variable; declare it CONSTANT if its value never changes
    ----'
    ");
}

// A FUNCTION's or METHOD's variables take their values at each call, in the
// order they are declared: one initialized from a later one, or from
// itself, read 0.
#[rstest]
fn invalid_initializer_reads_a_variable_declared_after_it(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : INT
VAR_INPUT n : INT; END_VAR
VAR
    a : INT := b;
    b : INT := n + 1;
    c : INT := c + 1;
    d : ARRAY[0..1] OF INT := [e, e];
    e : INT := 2;
END_VAR
    F := a + b + c + d[0] + e;
END_FUNCTION

FUNCTION_BLOCK Fb
    METHOD M : INT
    VAR x : INT := y; y : INT := 4; END_VAR
        M := x + y;
    END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0406] Error: read before it has its value
       ,-[ file:///test0.st:5:16 ]
       |
     5 |     a : INT := b;
       |                |
       |                `-- the initial value of 'a' reads 'b', declared after it
     6 |     b : INT := n + 1;
       |     |
       |     `-- 'b' is declared here
       |
       | Note: variables get their initial values in the order they are declared; declare 'b' before 'a'
    ---'
    [E0406] Error: read before it has its value
       ,-[ file:///test0.st:7:16 ]
       |
     7 |     c : INT := c + 1;
       |                |
       |                `-- the initial value of 'c' reads 'c' itself
       |
       | Note: an initial value cannot read the variable it initializes
    ---'
    [E0406] Error: read before it has its value
       ,-[ file:///test0.st:8:32 ]
       |
     8 |     d : ARRAY[0..1] OF INT := [e, e];
       |                                |
       |                                `-- the initial value of 'd' reads 'e', declared after it
     9 |     e : INT := 2;
       |     |
       |     `-- 'e' is declared here
       |
       | Note: variables get their initial values in the order they are declared; declare 'e' before 'd'
    ---'
    [E0406] Error: read before it has its value
        ,-[ file:///test0.st:16:20 ]
        |
     16 |     VAR x : INT := y; y : INT := 4; END_VAR
        |                    |  |
        |                    `----- the initial value of 'x' reads 'y', declared after it
        |                       |
        |                       `-- 'y' is declared here
        |
        | Note: variables get their initial values in the order they are declared; declare 'y' before 'x'
    ----'
    ");
}

// An initializer reads what has its value already: an input, a variable
// declared before it, and a CONSTANT, whose value is put in its place
// wherever it is declared. A REF() takes an address, not a value.
#[rstest]
fn valid_initializer_reads_what_has_its_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION G : INT
VAR_INPUT n : INT; END_VAR
VAR
    a : INT := n + K;
    b : INT := a * 2;
    p : REF_TO INT := REF(c);
    c : INT := b;
END_VAR
VAR CONSTANT K : INT := 3; END_VAR
    G := c + p^;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// Only what gets its value at entry is ordered. An input, an in-out and an
// external hold theirs before the first initializer runs, wherever their
// section is written; an output with an initial value takes it in its turn.
#[rstest]
fn invalid_initializer_reads_an_output_declared_after_it(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL g : INT := 7; END_VAR
END_CONFIGURATION

FUNCTION Sections : INT
VAR
    a : INT := n;
    b : INT := io;
    c : INT := g;
    d : INT := o;
END_VAR
VAR_INPUT n : INT; END_VAR
VAR_IN_OUT io : INT; END_VAR
VAR_EXTERNAL g : INT; END_VAR
VAR_OUTPUT o : INT := 5; END_VAR
    Sections := a + b + c + d;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0406] Error: read before it has its value
        ,-[ file:///test0.st:11:16 ]
        |
     11 |     d : INT := o;
        |                |
        |                `-- the initial value of 'd' reads 'o', declared after it
        |
     16 | VAR_OUTPUT o : INT := 5; END_VAR
        |            |
        |            `-- 'o' is declared here
        |
        | Note: variables get their initial values in the order they are declared; declare 'o' before 'd'
    ----'
    ");
}

// A `^` on a reference set to REF(c) reads c, and a call on an instance
// reads the instance: each before it has its value.
#[rstest]
fn invalid_initializer_reads_through_a_reference_or_a_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR v : INT := 9; END_VAR
    METHOD Get : INT
        Get := v;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION ThroughRef : INT
VAR
    p : REF_TO INT := REF(c);
    q : INT := p^;
    c : INT := 5;
END_VAR
    ThroughRef := q;
END_FUNCTION

FUNCTION LaterInstance : INT
VAR
    a : INT := inst.Get();
    inst : Fb;
END_VAR
    LaterInstance := a;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0406] Error: read before it has its value
        ,-[ file:///test0.st:12:16 ]
        |
     12 |     q : INT := p^;
        |                |
        |                `-- the initial value of 'q' reads 'c', declared after it, through 'p'
     13 |     c : INT := 5;
        |     |
        |     `-- 'c' is declared here
        |
        | Note: variables get their initial values in the order they are declared; declare 'c' before 'q'
    ----'
    [E0406] Error: read before it has its value
        ,-[ file:///test0.st:20:16 ]
        |
     20 |     a : INT := inst.Get();
        |                ^^|^
        |                  `--- the initial value of 'a' reads 'inst', declared after it
     21 |     inst : Fb;
        |     ^^|^
        |       `--- 'inst' is declared here
        |
        | Note: variables get their initial values in the order they are declared; declare 'inst' before 'a'
    ----'
    ");
}

/// A comparison is no constant, however constant its operands: `T#2s >
/// LT#1s` is refused as a member default, so no static initializer reaches
/// the comparison lowering and the folder has no comparison to get wrong.
/// The operands' widening is a different fold, which `LTIME := T#2s` takes.
#[rstest]
fn an_fb_member_default_from_a_comparison_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR
    later : BOOL := T#2s > LT#1s;
    wide : LTIME := T#2s;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value is not constant
       ,-[ file:///test0.st:4:21 ]
       |
     4 |     later : BOOL := T#2s > LT#1s;
       |                     ^^^^^^|^^^^^
       |                           `------- this initial value must be a constant: it is fixed before the program runs
    ---'
    ");
}
