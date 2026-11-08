// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `a := b := e`: the last place is written the value, then each other the
//! value of the place to its right, and each link is checked as an
//! assignment of its own. A place written with an operator, `a += b := e`,
//! is written its operation over that value, checked as written out.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// A chain writes any place an assignment writes, and accepts what each of
/// its links would accept alone.
#[rstest]
#[case::one_type("a := b := c := 1;")]
#[case::widened_from_right_to_left("r := d := i := 1;")]
#[case::a_bit_and_an_element("w.%X3 := flags[i] := flag := TRUE;")]
#[case::a_named_value("m1 := m2 := Mode#Run;")]
#[case::a_structure("p1 := p2 := p3;")]
#[case::a_reference("r2 := r1 := REF(x);")]
#[case::a_dereference("r1 := REF(x); r1^ := a := 3;")]
#[case::an_input_of_an_instance("fb.go := flag := TRUE;")]
#[case::a_string_into_a_longer_one("s80 := s4 := 'abcd';")]
#[case::the_result("f := a := 2;")]
fn valid_chains(#[case] chain: &str, mut with_db: RootDatabase) {
    let source = format!(
        r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
TYPE Mode : (Idle, Run); END_TYPE

FUNCTION_BLOCK Fb
VAR_INPUT go : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR
    a, b, c, i : INT; d : DINT; r : LREAL;
    w : WORD; flags : ARRAY[0..3] OF BOOL; flag : BOOL;
    m1, m2 : Mode; p1, p2, p3 : Pt; x : INT; r1, r2 : REF_TO INT;
    s4 : STRING[4]; s80 : STRING; fb : Fb;
END_VAR
    {chain}
    f := a;
END_FUNCTION
"#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// A place of a chain can be written with an operator: `id := next += 1`
/// counts, `a += b := 1` adds what it wrote, and each such link accepts what
/// it would written out alone.
#[rstest]
#[case::on_the_last_place("id := next += 1;")]
#[case::on_the_first_place("a += b := 1;")]
#[case::on_every_place("a += b -= c *= i /= 2;")]
#[case::a_wider_place("d += i := 7;")]
#[case::widened_after_it("r := i *= 2;")]
#[case::reals("x *= y /= 2.0;")]
#[case::an_element_and_a_field("arr[i] += p.x := 3;")]
#[case::a_dereference("r1 := REF(a); r1^ += b := 2;")]
#[case::a_duration("t1 += t2 := T#1s;")]
#[case::the_result("f += a := 2;")]
fn valid_chains_with_operators(#[case] chain: &str, mut with_db: RootDatabase) {
    let source = format!(
        r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION f : INT
VAR
    a, b, c, i : INT; d, id, next : DINT; r : LREAL; x, y : REAL;
    arr : ARRAY[0..3] OF INT; p : Pt; r1 : REF_TO INT; t1, t2 : TIME;
END_VAR
    {chain}
    f := a;
END_FUNCTION
"#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// A link with an operator is checked as written out, `a := a + d` for
/// `a += d`: a result its place cannot take is refused on the operator and
/// the value, with the operation written out in a statement of its own, and
/// so is an operator its type has not.
#[rstest]
fn invalid_a_link_with_an_operator(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR a : INT; d : DINT; r : REAL; s, t : STRING; on, off : BOOL; END_VAR
    a += d := 5;
    d := a += r;
    s += t := 'x';
    on += off := TRUE;
    f := a;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:7 ]
       |
     3 | VAR a : INT; d : DINT; r : REAL; s, t : STRING; on, off : BOOL; END_VAR
       |     |
       |     `-- 'a' is declared here
     4 |     a += d := 5;
       |       ^^|^
       |         `--- expected 'INT', got 'DINT'
       |
       | Help: write 'a := DINT_TO_INT(a + d)' in a statement of its own
       |
       | Note: 'a += d' is 'a := a + d'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:12 ]
       |
     3 | VAR a : INT; d : DINT; r : REAL; s, t : STRING; on, off : BOOL; END_VAR
       |     |
       |     `-- 'a' is declared here
       |
     5 |     d := a += r;
       |            ^^|^
       |              `--- expected 'INT', got 'REAL'
       |
       | Help: write 'a := REAL_TO_INT(a + r)' in a statement of its own
       |
       | Note: 'a += r' is 'a := a + r'
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:6:7 ]
       |
     3 | VAR a : INT; d : DINT; r : REAL; s, t : STRING; on, off : BOOL; END_VAR
       |                                  |
       |                                  `-- 's' is declared here
       |
     6 |     s += t := 'x';
       |       ^^|^
       |         `--- operator '+' cannot be applied to type 'STRING'
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:7:8 ]
       |
     3 | VAR a : INT; d : DINT; r : REAL; s, t : STRING; on, off : BOOL; END_VAR
       |                                                 ^|
       |                                                  `-- 'on' is declared here
       |
     7 |     on += off := TRUE;
       |        ^^^|^^
       |           `---- operator '+' cannot be applied to type 'BOOL'
    ---'
    ");
}

/// A BOOL written a quotient, `flag := a /= b`, most likely meant `<>`,
/// "not equal" as other languages write it: the fix compares, and a note
/// says what `/=` does. Through another place, the quotient was meant.
#[rstest]
fn invalid_a_quotient_into_a_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR a, b, q : INT; r : REAL; flag : BOOL; END_VAR
    flag := a /= b;
    flag := r /= 2.0;
    flag := q := a /= b;
    f := a;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:13 ]
       |
     3 | VAR a, b, q : INT; r : REAL; flag : BOOL; END_VAR
       |                              ^^|^
       |                                `--- 'flag' is declared here
     4 |     flag := a /= b;
       |             |
       |             `-- expected 'BOOL', got 'INT'
       |
       | Help: replace '/=' with '<>'
       |
       | Note 1: a place in a chain takes the value of the place to its right
       |
       | Note 2: 'a /= b' is 'a := a / b'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:13 ]
       |
     3 | VAR a, b, q : INT; r : REAL; flag : BOOL; END_VAR
       |                              ^^|^
       |                                `--- 'flag' is declared here
       |
     5 |     flag := r /= 2.0;
       |             |
       |             `-- expected 'BOOL', got 'REAL'
       |
       | Help: replace '/=' with '<>'
       |
       | Note 1: a place in a chain takes the value of the place to its right
       |
       | Note 2: 'r /= 2.0' is 'r := r / 2.0'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:13 ]
       |
     3 | VAR a, b, q : INT; r : REAL; flag : BOOL; END_VAR
       |                              ^^|^
       |                                `--- 'flag' is declared here
       |
     6 |     flag := q := a /= b;
       |             |
       |             `-- expected 'BOOL', got 'INT'
       |
       | Help: assign 'flag' in a statement of its own
       |
       | Note: a place in a chain takes the value of the place to its right
    ---'
    ");
}

/// The fix for `flag := a /= b` replaces the operator, and the statement
/// then compares.
#[rstest]
fn the_quotient_fix_compares(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR a, b : INT; flag : BOOL; END_VAR
    flag := a /= b;
    f := a;
END_FUNCTION
"#;
    let file = crate::tests::utils::add_source(&mut with_db, source);
    let edits: Vec<_> = hir::check::diagnostics_for_file(&with_db, file)
        .iter()
        .flat_map(|d| d.fixes().to_vec())
        .flat_map(|fix| {
            let changes = fix.edit.and_then(|edit| edit.changes).unwrap_or_default();
            changes.into_values().flatten()
        })
        .collect();
    let [edit] = edits.as_slice() else {
        panic!("one edit: {edits:?}");
    };
    assert_eq!(edit.range.start.line, edit.range.end.line);
    let line = source
        .lines()
        .nth(edit.range.start.line as usize)
        .expect("the line");
    let (start, end) = (
        edit.range.start.character as usize,
        edit.range.end.character as usize,
    );
    let fixed = format!("{}{}{}", &line[..start], edit.new_text, &line[end..]);
    assert_eq!(fixed, "    flag := a <> b;");
}

/// A link whose place cannot take the value of the place to its right is
/// refused there, as `count := total` would be. No cast can be written
/// around a place, so the help is to assign it apart.
#[rstest]
fn invalid_a_link_of_another_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR count : INT; total : DINT; i : INT; r : REAL; END_VAR
    count := total := 0;
    total := count := 0;
    i := r := 2.5;
    f := count;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:14 ]
       |
     3 | VAR count : INT; total : DINT; i : INT; r : REAL; END_VAR
       |     ^^|^^
       |       `---- 'count' is declared here
     4 |     count := total := 0;
       |              ^^|^^
       |                `---- expected 'INT', got 'DINT'
       |
       | Help: assign 'count' in a statement of its own
       |
       | Note: a place in a chain takes the value of the place to its right
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:10 ]
       |
     3 | VAR count : INT; total : DINT; i : INT; r : REAL; END_VAR
       |                                |
       |                                `-- 'i' is declared here
       |
     6 |     i := r := 2.5;
       |          |
       |          `-- expected 'INT', got 'REAL'
       |
       | Help: assign 'i' in a statement of its own
       |
       | Note: a place in a chain takes the value of the place to its right
    ---'
    ");
}

/// Each place of a chain is checked as written: a CONSTANT anywhere in it, a
/// name that resolves to nothing, reported once, also where an operation
/// reads it.
#[rstest]
fn invalid_a_place_that_cannot_be_written(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR a : INT; END_VAR
VAR CONSTANT K : INT := 1; END_VAR
    a := K := 1;
    K := a := 1;
    a := nope := 1;
    K += a := 1;
    a += nope := 1;
    nope += a := 1;
    f := a;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:5:10 ]
       |
     4 | VAR CONSTANT K : INT := 1; END_VAR
       |              |
       |              `-- 'K' is declared CONSTANT here
     5 |     a := K := 1;
       |          |
       |          `-- cannot write to constant 'K'
       |
       | Help: copy it into a variable to change the copy
       |
       | Note: a CONSTANT keeps the value it is declared with
    ---'
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:6:5 ]
       |
     4 | VAR CONSTANT K : INT := 1; END_VAR
       |              |
       |              `-- 'K' is declared CONSTANT here
       |
     6 |     K := a := 1;
       |     |
       |     `-- cannot write to constant 'K'
       |
       | Help: copy it into a variable to change the copy
       |
       | Note: a CONSTANT keeps the value it is declared with
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:7:10 ]
       |
     7 |     a := nope := 1;
       |          ^^|^
       |            `--- no item 'nope' found in scope
    ---'
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:8:5 ]
       |
     4 | VAR CONSTANT K : INT := 1; END_VAR
       |              |
       |              `-- 'K' is declared CONSTANT here
       |
     8 |     K += a := 1;
       |     |
       |     `-- cannot write to constant 'K'
       |
       | Help: copy it into a variable to change the copy
       |
       | Note: a CONSTANT keeps the value it is declared with
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:9:10 ]
       |
     9 |     a += nope := 1;
       |          ^^|^
       |            `--- no item 'nope' found in scope
    ---'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:10:5 ]
        |
     10 |     nope += a := 1;
        |     ^^|^
        |       `--- no item 'nope' found in scope
    ----'
    ");
}

/// The value is the last place's: a literal too long is measured against it.
#[rstest]
fn invalid_a_literal_over_the_last_place(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR s4 : STRING[4]; s80 : STRING; END_VAR
    s80 := s4 := 'abcdefgh';
    f := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0314] Error: STRING literal over its capacity
       ,-[ file:///test0.st:4:18 ]
       |
     4 |     s80 := s4 := 'abcdefgh';
       |                  ^^^^^|^^^^
       |                       `------ the literal is 8 bytes, over the capacity of 4
       |
       | Help: declare it STRING[8], or shorten the literal
    ---'
    ");
}

/// A constant outside the last place's subrange is refused as it is in
/// `sr := 20`. Through a place, `sr := i := 20`, the value is known only as
/// it runs, and the store checks it then.
#[rstest]
fn invalid_a_value_outside_the_last_places_subrange(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR sr : INT(0..10); i : INT; END_VAR
    i := sr := 20;
    sr := i := 20;
    f := i;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside the subrange
       ,-[ file:///test0.st:4:16 ]
       |
     4 |     i := sr := 20;
       |                ^|
       |                 `-- value 20 is outside the subrange 0..10
       |
       | Note: the declared range only admits values from 0 to 10
    ---'
    ");
}

/// A reference to a call's own storage is kept by every place of the chain:
/// one that outlives the call is refused, and so is the result.
#[rstest]
fn invalid_a_reference_to_a_local_kept_by_a_chain(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION_BLOCK Keeper
VAR kept : REF_TO INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION stores : INT
VAR_IN_OUT k : Keeper; END_VAR
VAR x : INT; r : REF_TO INT; END_VAR
    k.kept := r := REF(x);
    stores := 0;
END_FUNCTION

FUNCTION returns : PInt
VAR x : INT; r : PInt; END_VAR
    returns := r := REF(x);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:11:20 ]
        |
     10 | VAR x : INT; r : REF_TO INT; END_VAR
        |     |
        |     `-- 'x' is per-call storage, declared here
     11 |     k.kept := r := REF(x);
        |     ^^^|^^         ^^^|^^
        |        `------------------- 'k.kept' outlives the call
        |                       |
        |                       `---- reference to 'x' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:17:21 ]
        |
     16 | VAR x : INT; r : PInt; END_VAR
        |     |
        |     `-- 'x' is per-call storage, declared here
     17 |     returns := r := REF(x);
        |                     ^^^|^^
        |                        `---- reference to 'x' outlives the call that owns it
        |
        | Help: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
    ----'
    ");
}

/// Every place of a chain holds what the value holds: NULL in both here.
#[rstest]
fn invalid_null_through_a_chain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR x : INT; r1, r2 : REF_TO INT; END_VAR
    r1 := REF(x);
    r2 := REF(x);
    r1 := r2 := NULL;
    x := r1^ + r2^;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0902] Error: possibly null dereference
       ,-[ file:///test0.st:7:10 ]
       |
     6 |     r1 := r2 := NULL;
       |     ^^^^^^^^|^^^^^^^
       |             `--------- 'r1' is set to NULL here
     7 |     x := r1^ + r2^;
       |          ^|
       |           `-- 'r1' is dereferenced and is NULL
    ---'
    [E0902] Error: possibly null dereference
       ,-[ file:///test0.st:7:16 ]
       |
     6 |     r1 := r2 := NULL;
       |     ^^^^^^^^|^^^^^^^
       |             `--------- 'r2' is set to NULL here
     7 |     x := r1^ + r2^;
       |                ^|
       |                 `-- 'r2' is dereferenced and is NULL
    ---'
    ");
}

/// An instance with a member declared `AT %Q*` copied by a chain: each link
/// is a copy that would write over the member (E1427).
#[rstest]
fn invalid_a_located_member_copied_by_a_chain(mut with_db: RootDatabase) {
    let source = r#"
CLASS Sensor
VAR raw AT %Q* : INT; END_VAR
END_CLASS

PROGRAM P
VAR a : Sensor; b : Sensor; c : Sensor; END_VAR
    b := c := a;
END_PROGRAM

CONFIGURATION Cfg
VAR_CONFIG
    Res.P1.a.raw AT %QW0 : INT;
    Res.P1.b.raw AT %QW1 : INT;
    Res.P1.c.raw AT %QW2 : INT;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1427] Error: located variable overwritten
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     b := c := a;
       |     |
       |     `-- the copy writes over 'raw', declared AT %Q*
       |
       | Help: copy the other members one by one, or share the instance through a VAR_IN_OUT
       |
       | Note: it points at the channel VAR_CONFIG gives its instance, and the copy would point it at the other instance's
    ---'
    [E1427] Error: located variable overwritten
       ,-[ file:///test0.st:8:10 ]
       |
     8 |     b := c := a;
       |          |
       |          `-- the copy writes over 'raw', declared AT %Q*
       |
       | Help: copy the other members one by one, or share the instance through a VAR_IN_OUT
       |
       | Note: it points at the channel VAR_CONFIG gives its instance, and the copy would point it at the other instance's
    ---'
    ");
}

/// A chain is a statement: not an argument, a FOR's start or a condition,
/// and what follows the second `:=` is the chain's, operators included.
/// With no value after its last `:=`, it is the empty right-hand side.
#[rstest]
fn invalid_a_chain_outside_a_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION g : INT
VAR_INPUT a : INT; END_VAR
    g := a;
END_FUNCTION

FUNCTION f : INT
VAR a, b, c, i, j, k : INT; on, x, y : BOOL; END_VAR
    f := g(a := b := 1);
    FOR i := j := 0 TO 10 DO END_FOR;
    IF on := x := y THEN f := 1; END_IF;
    a := b := ;
    f := g(a := b := c += 1);
    FOR i := j := k += 1 TO 10 DO END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0032] Error: chain of assignments outside a statement
       ,-[ file:///test0.st:9:19 ]
       |
     9 |     f := g(a := b := 1);
       |                   ^|
       |                    `-- a second ':=' chains an assignment
       |
       | Help: assign 'b' in a statement before the call
       |
       | Note: a chain of assignments is a statement of its own
    ---'
    [E0032] Error: chain of assignments outside a statement
        ,-[ file:///test0.st:10:16 ]
        |
     10 |     FOR i := j := 0 TO 10 DO END_FOR;
        |                ^|
        |                 `-- a second ':=' chains an assignment
        |
        | Help: assign 'j' in a statement before the FOR
        |
        | Note: a chain of assignments is a statement of its own
    ----'
    [E0012] Error: assignment sign ':=' in a condition
        ,-[ file:///test0.st:11:8 ]
        |
     11 |     IF on := x := y THEN f := 1; END_IF;
        |        ^^^|^^^
        |           `----- a condition compares with '=', not ':='
        |
        | Help: replace ':=' with '='
    ----'
    [E0001] Error: syntax error
        ,-[ file:///test0.st:11:16 ]
        |
     11 |     IF on := x := y THEN f := 1; END_IF;
        |                ^^|^
        |                  `--- unexpected token(s): ':= y'
    ----'
    [E0005] Error: empty right-hand side of assignment
        ,-[ file:///test0.st:12:12 ]
        |
     12 |     a := b := ;
        |            ^|
        |             `-- the assignment has no right-hand side
    ----'
    [E0032] Error: chain of assignments outside a statement
        ,-[ file:///test0.st:13:19 ]
        |
     13 |     f := g(a := b := c += 1);
        |                   ^|
        |                    `-- a second ':=' chains an assignment
        |
        | Help: assign 'b' in a statement before the call
        |
        | Note: a chain of assignments is a statement of its own
    ----'
    [E0032] Error: chain of assignments outside a statement
        ,-[ file:///test0.st:14:16 ]
        |
     14 |     FOR i := j := k += 1 TO 10 DO END_FOR;
        |                ^|
        |                 `-- a second ':=' chains an assignment
        |
        | Help: assign 'j' in a statement before the FOR
        |
        | Note: a chain of assignments is a statement of its own
    ----'
    ");
}
