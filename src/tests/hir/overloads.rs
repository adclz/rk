//! FUNCTION overload resolution — a call binds to the same-name overload whose
//! signature (ordered parameter types, see `function_signature`) matches the
//! call's argument types. Overloads that are exact-on-every-arg win; when an
//! argument fits several by widening and none is exact, the call is ambiguous
//! (E0809) rather than guessed. Identical signatures are rejected as duplicates
//! (see `duplicates.rs`).

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// A 2-arg call binds to the 2-arg overload even though the 1-arg overload is
// declared first (and would be the naive first-match). The return types differ
// (INT vs STRING), so the assignment only type-checks if the *right* overload
// was selected.
#[rstest]
fn call_binds_to_arity_matching_overload(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : STRING
VAR_INPUT a : INT; b : INT; END_VAR
    foo := 'x';
END_FUNCTION

FUNCTION test : INT
VAR
    i : INT;
    s : STRING;
END_VAR
    i := foo(1);        // -> foo/1 : INT
    s := foo(1, 2);     // -> foo/2 : STRING
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// Selecting the wrong overload would surface as a type error: if `foo(1, 2)`
// bound to the 1-arg INT overload, assigning its result to a STRING fails.
#[rstest]
fn wrong_overload_would_type_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : STRING
VAR_INPUT a : INT; b : INT; END_VAR
    foo := 'x';
END_FUNCTION

FUNCTION test : INT
VAR i : INT; END_VAR
    i := foo(1, 2);     // foo/2 returns STRING -> STRING := INT is a type error
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:10 ]
        |
     13 | VAR i : INT; END_VAR
        |     |
        |     `-- type is declared by variable 'i' here
     14 |     i := foo(1, 2);     // foo/2 returns STRING -> STRING := INT is a type error
        |          ^^^^|^^^^
        |              `------ expected 'INT', got 'STRING'
    ----'
    ");
}

// Same arity, different parameter TYPE is a legal overload set (not a
// duplicate), and a call binds by argument type. Distinct return types (INT vs
// BOOL) make a wrong pick a type error, so a clean run proves correct selection.
#[rstest]
fn overload_by_parameter_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION describe : INT
VAR_INPUT x : INT; END_VAR
    describe := 1;
END_FUNCTION

FUNCTION describe : BOOL
VAR_INPUT x : REAL; END_VAR
    describe := TRUE;
END_FUNCTION

FUNCTION test : INT
VAR i : INT; r : REAL; n : INT; b : BOOL; END_VAR
    n := describe(i);   // -> describe(INT) : INT
    b := describe(r);   // -> describe(REAL) : BOOL
    test := n;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// When an argument fits several overloads and none is an exact match, the
// compiler refuses to guess and reports E0809. `pick(1)` — the untyped literal
// widens to both DINT and LINT.
#[rstest]
fn ambiguous_overload_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION pick : INT
VAR_INPUT x : DINT; END_VAR
    pick := 1;
END_FUNCTION

FUNCTION pick : INT
VAR_INPUT x : LINT; END_VAR
    pick := 2;
END_FUNCTION

FUNCTION test : INT
    test := pick(1);
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:13:13 ]
        |
      2 | ,---> FUNCTION pick : INT
        : :
      5 | |---> END_FUNCTION
        | |
        | `-------------------- candidate overload declared here
        |
      7 |   ,-> FUNCTION pick : INT
        :   :
     10 |   |-> END_FUNCTION
        |   |
        |   `------------------ candidate overload declared here
        |
     13 |           test := pick(1);
        |                   ^^|^
        |                     `--- call to 'pick' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// An argument count with no matching overload keeps the first-match, so the
// ordinary arity error still surfaces (no silent success).
#[rstest]
fn no_matching_arity_reports_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : INT
VAR_INPUT a : INT; b : INT; END_VAR
    foo := a + b;
END_FUNCTION

FUNCTION test : INT
VAR x : INT; END_VAR
    test := foo(x, x, x);   // no 3-arg overload
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0801] Error: function call parameter mismatch
        ,-[ file:///test0.st:14:13 ]
        |
     14 |     test := foo(x, x, x);   // no 3-arg overload
        |             ^|^
        |              `--- no overload of 'foo' takes 3 parameters
    ----'
    ");
}

/// An argument TYPE no overload accepts is E0810, naming what was passed and
/// what each overload takes. The first overload used to stand in and report
/// its own parameter mismatch: "expected 'CHAR', got 'DATE'" for a date
/// assertion, a type nobody wrote.
#[rstest]
fn no_matching_type_names_the_overloads(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION take : INT
VAR_INPUT v : INT; END_VAR
    take := v;
END_FUNCTION

FUNCTION take : INT
VAR_INPUT v : REAL; END_VAR
    take := 2;
END_FUNCTION

FUNCTION caller : INT
    caller := take('text');
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0810] Error: no matching overload
        ,-[ file:///test0.st:13:15 ]
        |
      2 |   ,-> FUNCTION take : INT
        :   :
      5 |   |-> END_FUNCTION
        |   |
        |   `------------------ overload accepting (INT)
        |
      7 | ,---> FUNCTION take : INT
        : :
     10 | |---> END_FUNCTION
        | |
        | `-------------------- overload accepting (REAL)
        |
     13 |           caller := take('text');
        |                     ^^|^
        |                       `--- no overload of 'take' accepts (STRING)
    ----'
    ");
}

/// A set where only some overloads take the argument count: the types are
/// what failed, so it is E0810 listing every overload, not the arity error.
#[rstest]
fn no_matching_type_in_a_mixed_arity_set(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION take : INT
VAR_INPUT v : INT; END_VAR
    take := v;
END_FUNCTION

FUNCTION take : INT
VAR_INPUT a : INT; b : INT; END_VAR
    take := a + b;
END_FUNCTION

FUNCTION caller : INT
    caller := take(TRUE);
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0810] Error: no matching overload
        ,-[ file:///test0.st:13:15 ]
        |
      2 |   ,-> FUNCTION take : INT
        :   :
      5 |   |-> END_FUNCTION
        |   |
        |   `------------------ overload accepting (INT)
        |
      7 | ,---> FUNCTION take : INT
        : :
     10 | |---> END_FUNCTION
        | |
        | `-------------------- overload accepting (INT, INT)
        |
     13 |           caller := take(TRUE);
        |                     ^^|^
        |                       `--- no overload of 'take' accepts (BOOL)
    ----'
    ");
}

/// An argument whose type comes from an ADJUSTMENT — array indexing, struct
/// field access, dereference — must be classified by its adjusted type.
///
/// HIR records `arr[0]` as the ARRAY type with the element type in the
/// adjustment, so overload resolution reading the RAW type saw an array,
/// matched no elementary parameter, and silently selected an unrelated
/// overload: `ASSERT_EQ(arr[0], 5)` picked the CHAR one and failed with
/// "expected 'CHAR', got 'INT'".
#[rstest]
fn overload_selected_from_adjusted_argument_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION take : INT
VAR_INPUT v : INT; END_VAR
    take := 1;
END_FUNCTION

FUNCTION take : INT
VAR_INPUT v : REAL; END_VAR
    take := 2;
END_FUNCTION

FUNCTION caller : INT
VAR
    arr : ARRAY[0..3] OF INT;
    reals : ARRAY[0..3] OF REAL;
    s : STRUCT a : INT; b : REAL; END_STRUCT;
END_VAR
    caller := take(arr[0]) + take(reals[0]) + take(s.a) + take(s.b);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Dominance: an overload at least as good on every argument and strictly
/// better on one wins. `(REAL, REAL)` beats `(LREAL, LREAL)` for a
/// `(REAL, INT)` call — exact beats widened on the first argument, tie on the
/// second. The result types differ (REAL vs LREAL), so the clean assignment
/// to a REAL proves WHICH overload was picked, not merely that one was.
#[rstest]
fn dominant_overload_wins(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION scale : REAL
VAR_INPUT a : REAL; b : REAL; END_VAR
    scale := a;
END_FUNCTION

FUNCTION scale : LREAL
VAR_INPUT a : LREAL; b : LREAL; END_VAR
    scale := a;
END_FUNCTION

FUNCTION caller : REAL
VAR x : REAL; n : INT; END_VAR
    caller := scale(x, n);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A literal argument counts as exact against its DEFAULT type: `2.0` is a
/// REAL exactly and an LREAL only by widening, so `(REAL, REAL)` dominates
/// even with an INT literal alongside.
#[rstest]
fn literal_defaults_drive_dominance(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION scale : REAL
VAR_INPUT a : REAL; b : REAL; END_VAR
    scale := a;
END_FUNCTION

FUNCTION scale : LREAL
VAR_INPUT a : LREAL; b : LREAL; END_VAR
    scale := a;
END_FUNCTION

FUNCTION caller : REAL
    caller := scale(2.0, 10);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Incomparable candidates stay ambiguous: each is better on a different
/// argument, and dominance never picks by majority.
#[rstest]
fn incomparable_overloads_stay_ambiguous(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION mix : INT
VAR_INPUT a : INT; b : LREAL; END_VAR
    mix := 1;
END_FUNCTION

FUNCTION mix : INT
VAR_INPUT a : DINT; b : REAL; END_VAR
    mix := 2;
END_FUNCTION

FUNCTION caller : INT
VAR i : INT; r : REAL; END_VAR
    caller := mix(i, r);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:14:15 ]
        |
      2 | ,---> FUNCTION mix : INT
        : :
      5 | |---> END_FUNCTION
        | |
        | `-------------------- candidate overload declared here
        |
      7 |   ,-> FUNCTION mix : INT
        :   :
     10 |   |-> END_FUNCTION
        |   |
        |   `------------------ candidate overload declared here
        |
     14 |           caller := mix(i, r);
        |                     ^|^
        |                      `--- call to 'mix' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

/// Two candidates widened everywhere tie exactly; nothing dominates, so the
/// call is ambiguous and asks for a cast.
#[rstest]
fn equally_widened_overloads_stay_ambiguous(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION up : DINT
VAR_INPUT a : DINT; END_VAR
    up := 1;
END_FUNCTION

FUNCTION up : LREAL
VAR_INPUT a : LREAL; END_VAR
    up := 2;
END_FUNCTION

FUNCTION caller : INT
VAR s : SINT; END_VAR
    caller := up(s);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:14:15 ]
        |
      2 |   ,-> FUNCTION up : DINT
        :   :
      5 |   |-> END_FUNCTION
        |   |
        |   `------------------ candidate overload declared here
        |
      7 | ,---> FUNCTION up : LREAL
        : :
     10 | |---> END_FUNCTION
        | |
        | `-------------------- candidate overload declared here
        |
     14 |           caller := up(s);
        |                     ^|
        |                      `-- call to 'up' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// -- RETURN-directed overloads: same params, different returns ---------------

// The pair is legal (E0102 compares params AND return), and the consuming
// site's type picks: each assignment resolves its own overload.
#[rstest]
fn valid_return_overloads_pick_by_target(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR t : TIME; lt : LTIME; END_VAR
            t := G();
            lt := G();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An initializer's declared type directs the pick too.
#[rstest]
fn valid_return_overload_in_initializer(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR t : TIME := G(); END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// Assigning to the function's own name targets its return slot.
#[rstest]
fn valid_return_overload_into_return_slot(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : TIME
            fn1 := G();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// A site with no expected type cannot pick: an operator operand is consumed
// by the OPERATOR, not the assignment. Bind the call first, or cast.
#[rstest]
fn invalid_return_overload_without_context(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR t : TIME; END_VAR
            t := G() + T#1ms;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:10:18 ]
        |
      2 |   ,->         FUNCTION G : TIME
        :   :
      4 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      5 | ,--->         FUNCTION G : LTIME
        : :
      7 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     10 |                   t := G() + T#1ms;
        |                        |
        |                        `-- call to 'G' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// A zero-argument call over fully-defaulted DISTINCT-param overloads used to
// pick the last candidate silently (the all-exact slot is vacuously true for
// the empty argument tuple); it is ambiguous and says so now.
#[rstest]
fn invalid_zero_arg_defaulted_overloads_ambiguous(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION H : INT
        VAR_INPUT a : INT := 1; END_VAR
            H := 1;
        END_FUNCTION
        FUNCTION H : INT
        VAR_INPUT b : REAL := 1.0; END_VAR
            H := 2;
        END_FUNCTION
        FUNCTION fn1 : INT
            fn1 := H();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:11:20 ]
        |
      2 |   ,->         FUNCTION H : INT
        :   :
      5 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      6 | ,--->         FUNCTION H : INT
        : :
      9 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     11 |                   fn1 := H();
        |                          |
        |                          `-- call to 'H' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// The WHILE-condition shape: one ambiguous call, ONE diagnostic.
#[rstest]
fn invalid_return_overload_in_while_condition(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR t : TIME; END_VAR
            WHILE G() - t < T#60ms DO
            END_WHILE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:10:19 ]
        |
      2 |   ,->         FUNCTION G : TIME
        :   :
      4 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      5 | ,--->         FUNCTION G : LTIME
        : :
      7 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     10 |                   WHILE G() - t < T#60ms DO
        |                         |
        |                         `-- call to 'G' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// A discarded statement call has no target: ambiguous, named candidates.
#[rstest]
fn invalid_return_overload_as_statement(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION fn1 : INT
            G();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
       ,-[ file:///test0.st:9:13 ]
       |
     2 |   ,->         FUNCTION G : TIME
       :   :
     4 |   |->         END_FUNCTION
       |   |
       |   `-------------------------- candidate overload declared here
     5 | ,--->         FUNCTION G : LTIME
       : :
     7 | |--->         END_FUNCTION
       | |
       | `---------------------------- candidate overload declared here
       |
     9 |                   G();
       |                   |
       |                   `-- call to 'G' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ---'
    ");
}

// An argument of an overloaded call: the inner pick would need the outer's
// choice and vice versa. The inner call errors rather than guessing.
#[rstest]
fn invalid_return_overload_as_overloaded_argument(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION G : TIME
            G := T#1ms;
        END_FUNCTION
        FUNCTION G : LTIME
            G := LTIME#2ms;
        END_FUNCTION
        FUNCTION f : INT
        VAR_INPUT x : TIME; END_VAR
            f := 1;
        END_FUNCTION
        FUNCTION f : INT
        VAR_INPUT x : LTIME; END_VAR
            f := 2;
        END_FUNCTION
        FUNCTION fn1 : INT
            fn1 := f(x := G());
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:17:27 ]
        |
      2 |   ,->         FUNCTION G : TIME
        :   :
      4 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      5 | ,--->         FUNCTION G : LTIME
        : :
      7 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     17 |                   fn1 := f(x := G());
        |                                 |
        |                                 `-- call to 'G' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// A call supplying every declared parameter beats a candidate padded out with
// defaults: `add(10)` is add/1, not an ambiguity with add/2's default.
#[rstest]
fn valid_full_arity_beats_defaulted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT := 5; END_VAR
            add := a + b;
        END_FUNCTION
        FUNCTION fn1 : INT
            fn1 := add(10);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// Two candidates each padding ONE default: neither is the no-padding
// candidate, so the arity preference has nothing to prefer — ambiguous by
// the same rule as the zero-argument set, never first-match.
#[rstest]
fn invalid_equal_default_padding_stays_ambiguous(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT := 1; END_VAR
            add := 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; c : REAL := 1.0; END_VAR
            add := 2;
        END_FUNCTION
        FUNCTION fn1 : INT
            fn1 := add(10);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:11:20 ]
        |
      2 |   ,->         FUNCTION add : INT
        :   :
      5 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      6 | ,--->         FUNCTION add : INT
        : :
      9 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     11 |                   fn1 := add(10);
        |                          ^|^
        |                           `--- call to 'add' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// A literal with NO exact candidate: `5` is INT by default, so REAL and
// LREAL are both promotions and neither dominates — E0809, the same answer
// a variable gets. A literal's default type is a compiler notion; this pins
// that it does not grow special promotion rules of its own.
#[rstest]
fn invalid_literal_with_only_promoted_candidates(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION conv : INT
        VAR_INPUT a : REAL; END_VAR
            conv := 1;
        END_FUNCTION
        FUNCTION conv : INT
        VAR_INPUT a : LREAL; END_VAR
            conv := 2;
        END_FUNCTION
        FUNCTION fn1 : INT
            fn1 := conv(5);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:11:20 ]
        |
      2 |   ,->         FUNCTION conv : INT
        :   :
      5 |   |->         END_FUNCTION
        |   |
        |   `-------------------------- candidate overload declared here
      6 | ,--->         FUNCTION conv : INT
        : :
      9 | |--->         END_FUNCTION
        | |
        | `---------------------------- candidate overload declared here
        |
     11 |                   fn1 := conv(5);
        |                          ^^|^
        |                            `--- call to 'conv' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

// THREE candidates where one dominates the other two, which are incomparable
// with EACH OTHER: the undominated set is a full filter, not a pairwise
// reduction, so the dominator wins regardless of declaration order. The
// (INT, INT) candidate is exact on both arguments; the mixed pair each widen
// one argument and would deadlock a reduction that met them first.
#[rstest]
fn valid_dominator_beats_incomparable_pair(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION mix : INT
        VAR_INPUT a : DINT; b : INT; END_VAR
            mix := 1;
        END_FUNCTION
        FUNCTION mix : INT
        VAR_INPUT a : INT; b : DINT; END_VAR
            mix := 2;
        END_FUNCTION
        FUNCTION mix : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            mix := 3;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR i : INT; j : INT; END_VAR
            fn1 := mix(i, j);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// THREE-way incomparable: each candidate is better on a different argument.
// The diagnostic counts and names all three.
#[rstest]
fn invalid_three_way_incomparable_names_all(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION mix : INT
        VAR_INPUT a : INT; b : LREAL; c : LREAL; END_VAR
            mix := 1;
        END_FUNCTION
        FUNCTION mix : INT
        VAR_INPUT a : LREAL; b : INT; c : LREAL; END_VAR
            mix := 2;
        END_FUNCTION
        FUNCTION mix : INT
        VAR_INPUT a : LREAL; b : LREAL; c : INT; END_VAR
            mix := 3;
        END_FUNCTION
        FUNCTION fn1 : INT
        VAR i : INT; j : INT; k : INT; END_VAR
            fn1 := mix(i, j, k);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0809] Error: ambiguous overloaded call
        ,-[ file:///test0.st:16:20 ]
        |
      2 | ,----->         FUNCTION mix : INT
        : :
      5 | |----->         END_FUNCTION
        | |
        | `------------------------------ candidate overload declared here
      6 |   ,--->         FUNCTION mix : INT
        :   :
      9 |   |--->         END_FUNCTION
        |   |
        |   `---------------------------- candidate overload declared here
     10 |     ,->         FUNCTION mix : INT
        :     :
     13 |     |->         END_FUNCTION
        |     |
        |     `-------------------------- candidate overload declared here
        |
     16 |                     fn1 := mix(i, j, k);
        |                            ^|^
        |                             `--- call to 'mix' is ambiguous: 3 overloads accept these arguments: disambiguate with an explicit cast
    ----'
    ");
}

/// The `ASSERT_EQ` shape: one overload per elementary type, called with a
/// bare literal. The literal's DEFAULT type is the exact match, so `'a'`
/// picks STRING and `0` picks INT - untyped does not mean ambiguous.
#[rstest]
fn a_bare_literal_picks_its_default_overload(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION eq : INT
VAR_INPUT v : CHAR; END_VAR
    eq := 1;
END_FUNCTION

FUNCTION eq : INT
VAR_INPUT v : STRING; END_VAR
    eq := 2;
END_FUNCTION

FUNCTION eq : INT
VAR_INPUT v : SINT; END_VAR
    eq := 3;
END_FUNCTION

FUNCTION eq : INT
VAR_INPUT v : INT; END_VAR
    eq := 4;
END_FUNCTION

FUNCTION caller : INT
    caller := eq('a') + eq('word') + eq(0);
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// With the default gone from the set, the literal takes the type that is
/// left - the same adoption the inference table performs, so the overload
/// chosen and the coercion that follows agree.
#[rstest]
fn a_bare_literal_adopts_the_only_type_offered(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION eq : INT
VAR_INPUT v : CHAR; END_VAR
    eq := 1;
END_FUNCTION

FUNCTION eq : INT
VAR_INPUT v : BOOL; END_VAR
    eq := 2;
END_FUNCTION

FUNCTION caller : INT
    caller := eq('a');
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// And a value no candidate can hold is still no match: the literal is
/// measured against each parameter, not waved through.
#[rstest]
fn no_matching_overload_for_an_oversized_bare_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION eq : INT
VAR_INPUT v : CHAR; END_VAR
    eq := 1;
END_FUNCTION

FUNCTION eq : INT
VAR_INPUT v : BOOL; END_VAR
    eq := 2;
END_FUNCTION

FUNCTION caller : INT
    caller := eq('word');
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0810] Error: no matching overload
        ,-[ file:///test0.st:13:15 ]
        |
      2 | ,---> FUNCTION eq : INT
        : :
      5 | |---> END_FUNCTION
        | |
        | `-------------------- overload accepting (CHAR)
        |
      7 |   ,-> FUNCTION eq : INT
        :   :
     10 |   |-> END_FUNCTION
        |   |
        |   `------------------ overload accepting (BOOL)
        |
     13 |           caller := eq('word');
        |                     ^|
        |                      `-- no overload of 'eq' accepts (STRING)
    ----'
    ");
}
