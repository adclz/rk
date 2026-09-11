use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_subrange_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: BOOL (0..5);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0701] Error: invalid subrange type
       ,-[ file:///test0.st:3:20 ]
       |
     3 |             Range: BOOL (0..5);
       |                    ^^|^
       |                      `--- Invalid subrange type 'BOOL'
       |
       | Note: only numeric integer types are allowed for SUBRANGE
    ---'
    ");
}

#[rstest]
fn invalid_start_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (-10..0);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:3:26 ]
       |
     3 |             Range: UINT (-10..0);
       |                          ^|^
       |                           `--- cannot infer '<integer>' to 'UINT': literal can not be negative
    ---'
    ");
}

#[rstest]
fn invalid_end_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..-5);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             Range: UINT (0..-5);
       |                             ^|
       |                              `-- cannot infer '<integer>' to 'UINT': literal can not be negative
    ---'
    ");
}

#[rstest]
fn invalid_subrange_value_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..5);
        END_TYPE

        FUNCTION fb1
            VAR
                test: Range;
            END_VAR

            test :=  -1 // -1 should not be allowed here (UINT)

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
        ,-[ file:///test0.st:11:22 ]
        |
      3 |             Range: UINT (0..5);
        |                    ^^^^^|^^^^^
        |                         `------- type is defined by 'Range' here
        |
     11 |             test :=  -1 // -1 should not be allowed here (UINT)
        |                      ^|
        |                       `-- cannot infer '<integer>' to 'UINT': literal can not be negative
    ----'
    ");
}

/// A subrange may be declared INLINE in a VAR block, not only through a TYPE
/// alias — other toolchains documents exactly this form. `var_decl`/`var_decl_init` did
/// not accept a subrange spec, so it was a syntax error.
#[rstest]
fn inline_subrange_declaration(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            i  : INT (-4095..4095);
            ui : UINT (0..10000);
            d  : INT (0..100) := 50;
        END_VAR
            i := 10;
        END_PROGRAM
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A constant outside the declared bounds is a COMPILE-time error (other toolchains
/// reports `i := 5000` on `INT (-4095..4095)`). Bounds are statically known, so
/// nothing waits for runtime.
#[rstest]
fn constant_above_subrange_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR p : INT (0..100); END_VAR
    p := 101;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     p := 101;
       |          ^|^
       |           `--- value 101 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

#[rstest]
fn constant_below_subrange_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR p : INT (0..100); END_VAR
    p := -1;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     p := -1;
       |          ^|
       |           `-- value -1 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

/// A value that does not fit the BASE type is reported once, by the base-type
/// check — no "outside subrange" pile-on for a single mistake.
#[rstest]
fn value_invalid_for_the_base_type_reports_once(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR u : UINT (0..5); END_VAR
    u := -1;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:10 ]
       |
     3 | VAR u : UINT (0..5); END_VAR
       |     |
       |     `-- type is declared by variable 'u' here
     4 |     u := -1;
       |          ^|
       |           `-- cannot infer '<integer>' to 'UINT': literal can not be negative
    ---'
    ");
}

/// Values at and inside the bounds are accepted.
#[rstest]
#[case("0")]
#[case("50")]
#[case("100")]
fn constant_within_subrange_is_accepted(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
        PROGRAM Main
        VAR p : INT (0..100); END_VAR
            p := {value};
        END_PROGRAM
        "#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// Bounds are enforced in every phase that assigns a value, not only in body
/// inference: an initializer is an assignment too.
#[rstest]
fn constant_outside_subrange_in_initializer_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR p : INT (0..100) := 200; END_VAR
    p := 0;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:3:25 ]
       |
     3 | VAR p : INT (0..100) := 200; END_VAR
       |                         ^|^
       |                          `--- value 200 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

/// ...and when the value is a call argument bound to a subrange parameter.
#[rstest]
fn constant_outside_subrange_as_call_argument_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION take : INT
VAR_INPUT p : INT (0..100); END_VAR
    take := p;
END_FUNCTION

PROGRAM Main
VAR r : INT; END_VAR
    r := take(p := 200);
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:9:20 ]
       |
     9 |     r := take(p := 200);
       |                    ^|^
       |                     `--- value 200 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

/// A subrange VALUE is usable wherever its base type is — `take := p` returns an
/// `INT (0..100)` as an `INT`. Coercion handled a subrange as the assignment
/// TARGET but not as the SOURCE, so reading one was rejected outright.
#[rstest]
fn subrange_value_is_assignable_to_its_base_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION take : INT
VAR_INPUT p : INT (0..100); END_VAR
    take := p;
END_FUNCTION

PROGRAM Main
VAR r : INT; END_VAR
    r := take(p := 50);
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A subrange may be an ARRAY's element type, and each element enforces the
/// bounds like any other subrange-typed target.
#[rstest]
fn array_of_subrange_enforces_bounds(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR a : ARRAY[0..3] OF INT (0..100); END_VAR
    a[0] := 50;
    a[1] := 200;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:5:13 ]
       |
     5 |     a[1] := 200;
       |             ^|^
       |              `--- value 200 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

/// A subrange works as a STRUCT field type.
#[rstest]
fn struct_field_subrange(mut with_db: RootDatabase) {
    let source = r#"
TYPE S : STRUCT pct : INT (0..100); END_STRUCT; END_TYPE
PROGRAM Main
VAR s : S; END_VAR
    s.pct := 50;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A function returning a subrange (named, as IEC requires for a return type)
/// and the result read into its base type.
#[rstest]
fn subrange_return_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pct : INT (0..100); END_TYPE

FUNCTION f : Pct
    f := 50;
END_FUNCTION

PROGRAM Main
VAR r : INT; END_VAR
    r := f();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_subrange_bound_not_constant(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR n : INT; x : INT (0..n); END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0703] Error: invalid subrange bound
       ,-[ file:///test0.st:3:34 ]
       |
     3 |         VAR n : INT; x : INT (0..n); END_VAR
       |                                  |
       |                                  `-- a subrange bound must evaluate to a constant at compile time
    ---'
    ");
}

// E0702 reads the FOLDED bounds, so a CONSTANT-bounded subrange still
// rejects an out-of-range initializer.
#[rstest]
fn invalid_value_outside_constant_bounds(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 5; END_VAR
        VAR x : INT (0..K) := 9; END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:4:31 ]
       |
     4 |         VAR x : INT (0..K) := 9; END_VAR
       |                               |
       |                               `-- value 9 is outside the subrange 0..5
       |
       | Note: the declared range only admits values from 0 to 5
    ---'
    ");
}

// E0704: the two ends of a by-reference binding must agree about the
// subrange, or the callee's writes go around the range check entirely.

#[rstest]
fn invalid_subrange_bound_to_plain_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : INT; END_VAR
            x := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR s : Small; END_VAR
            scribble(x := s);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
       ,-[ file:///test0.st:9:27 ]
       |
     9 |             scribble(x := s);
       |                           |
       |                           `-- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ---'
    ");
}

#[rstest]
fn invalid_plain_bound_to_subrange_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : Small; END_VAR
            x := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR n : INT; END_VAR
            scribble(x := n);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
       ,-[ file:///test0.st:9:27 ]
       |
     9 |             scribble(x := n);
       |                           |
       |                           `-- 'INT' binds by reference to 'Small (0..10)': the subrange must match exactly
    ---'
    ");
}

#[rstest]
fn invalid_plain_output_into_subrange(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        FUNCTION emitfn
        VAR_OUTPUT o : INT; END_VAR
            o := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR s : Small; END_VAR
            emitfn(o => s);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
       ,-[ file:///test0.st:9:25 ]
       |
     9 |             emitfn(o => s);
       |                         |
       |                         `-- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ---'
    ");
}

// The safe direction stays legal: a checked subrange output landing in a
// plain variable is already in range, and matching subranges alias nothing
// they do not both enforce.
#[rstest]
fn valid_subrange_output_into_plain_and_matching_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        FUNCTION emitfn
        VAR_OUTPUT o : Small; END_VAR
            o := 5;
        END_FUNCTION
        FUNCTION scribble
        VAR_IN_OUT x : Small; END_VAR
            x := 3;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR n : INT; s : Small; END_VAR
            emitfn(o => n);
            scribble(x := s);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// Two SUBRANGES with different bounds is where "must match exactly" does the
// work its wording implies: same base, both constrained, neither survives.
// The message spells both constraint sets so similarly-named types stay
// tellable apart.
#[rstest]
fn invalid_mismatched_subrange_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        TYPE Wider : INT (0..20); END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : Wider; END_VAR
            x := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR s : Small; END_VAR
            scribble(x := s);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:10:27 ]
        |
     10 |             scribble(x := s);
        |                           |
        |                           `-- 'Small (0..10)' binds by reference to 'Wider (0..20)': the subrange must match exactly
    ----'
    ");
}

// The check reads the ADJUSTED type: `r.f` and `a[1]` are a Rec and an ARRAY
// raw, and reading them raw would wave the binding through (bases differ, so
// the check would defer to coercion and accept).
#[rstest]
fn invalid_subrange_field_bound_to_plain_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        TYPE Rec : STRUCT f : Small; END_STRUCT END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : INT; END_VAR
            x := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR r : Rec; END_VAR
            scribble(x := r.f);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:10:27 ]
        |
     10 |             scribble(x := r.f);
        |                           ^|^
        |                            `--- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    ");
}

#[rstest]
fn invalid_subrange_element_bound_to_plain_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : INT; END_VAR
            x := 0;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR a : ARRAY[0..2] OF Small; END_VAR
            scribble(x := a[1]);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
       ,-[ file:///test0.st:9:27 ]
       |
     9 |             scribble(x := a[1]);
       |                           ^^|^
       |                             `--- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ---'
    ");
}

// And no false reject: a nested Small binds fine to a Small param.
#[rstest]
fn valid_nested_subrange_matching_inout(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        TYPE Rec : STRUCT f : Small; END_STRUCT END_TYPE
        FUNCTION scribble
        VAR_IN_OUT x : Small; END_VAR
            x := 3;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR r : Rec; a : ARRAY[0..2] OF Small; END_VAR
            scribble(x := r.f);
            scribble(x := a[1]);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// E0704 through REF(): a reference is a by-reference binding like a
// VAR_IN_OUT, so its pointee and the referenced variable must agree about the
// subrange. Before, `REF_TO INT := REF(s)` with `s : INT (0..10)` checked
// clean and `p^ := 500` went around the range check entirely.

#[rstest]
fn a_reference_must_match_the_subrange_of_its_target(mut with_db: RootDatabase) {
    let source = r#"
TYPE Small : INT (0..10); END_TYPE

FUNCTION g : INT
    VAR_INPUT p : REF_TO INT; END_VAR
    g := p^;
END_FUNCTION

FUNCTION f : INT
    VAR
        s : INT (0..10); n : Small; x : INT;
        p : REF_TO INT := REF(s);
        q : REF_TO Small;
    END_VAR
    p := REF(s);
    f := g(p := REF(s)) + g(REF(n));
    q := REF(n);
    p := q;
    q := REF(x);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:12:27 ]
        |
     12 |         p : REF_TO INT := REF(s);
        |                           ^^^|^^
        |                              `---- 'INT (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:15:10 ]
        |
     15 |     p := REF(s);
        |          ^^^|^^
        |             `---- 'INT (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:16:17 ]
        |
     16 |     f := g(p := REF(s)) + g(REF(n));
        |                 ^^^|^^
        |                    `---- 'INT (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:16:29 ]
        |
     16 |     f := g(p := REF(s)) + g(REF(n));
        |                             ^^^|^^
        |                                `---- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:18:10 ]
        |
     18 |     p := q;
        |          |
        |          `-- 'Small (0..10)' binds by reference to 'INT': the subrange must match exactly
    ----'
    [E0704] Error: subrange mismatch across a reference
        ,-[ file:///test0.st:19:10 ]
        |
     19 |     q := REF(x);
        |          ^^^|^^
        |             `---- 'INT' binds by reference to 'Small (0..10)': the subrange must match exactly
    ----'
    ");
}

#[rstest]
fn a_reference_with_equal_bounds_or_a_differing_base_is_not_e0804(mut with_db: RootDatabase) {
    // Equal bounds agree whatever the spelling; a differing BASE is E0301's
    // complaint alone, never stacked with E0704.
    let source = r#"
TYPE Small : INT (0..10); END_TYPE

FUNCTION f : REAL
    VAR
        s : INT (0..10); n : Small; x : INT;
        q : REF_TO Small; p : REF_TO INT; r : REF_TO REAL;
    END_VAR
    q := REF(n);
    q := REF(s);
    p := REF(x);
    p := NULL;
    r := REF(s);
    f := r^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:10 ]
        |
      7 |         q : REF_TO Small; p : REF_TO INT; r : REF_TO REAL;
        |                                           |
        |                                           `-- type is declared by variable 'r' here
        |
     13 |     r := REF(s);
        |          ^^^|^^
        |             `---- expected 'REF_TO REAL', got 'REF_TO INT (0..10)'
    ----'
    ");
}

/// A TYPE's own default is checked against its subrange like a variable's:
/// 200 is not a Pct.
#[rstest]
fn invalid_subrange_type_default_out_of_bounds(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Pct : INT (0..100) := 200;
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside subrange
       ,-[ file:///test0.st:3:35 ]
       |
     3 |             Pct : INT (0..100) := 200;
       |                                   ^|^
       |                                    `--- value 200 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}
