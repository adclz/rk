// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `x += e` is `x := x + e`, with the place `x` found once, and `-=`, `*=`,
//! `/=` alike: checked as written out, and a statement of its own.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// What `x := x + e` accepts, `x += e` accepts: an operand that widens, a
/// REAL, a duration, an element, a field, a dereference, an instance's
/// member, the result.
#[rstest]
#[case::each_operator("i += 1; i -= 2; i *= 3; i /= 4;")]
#[case::a_wider_place("d += i;")]
#[case::reals("r *= 2.0; r += i;")]
#[case::a_duration("t += T#1s; t -= T#500ms;")]
#[case::an_element("arr[i] += 1;")]
#[case::a_field("p.x -= 1;")]
#[case::a_dereference("ri := REF(i); ri^ *= 2;")]
#[case::an_instance_member("c.count += 1;")]
#[case::a_subrange("sr += 1;")]
#[case::the_result("f += 1;")]
fn valid_compound_assignments(#[case] stmt: &str, mut with_db: RootDatabase) {
    let source = format!(
        r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR
    i : INT; d : DINT; r : REAL; t : TIME; arr : ARRAY[0..3] OF INT;
    p : Pt; ri : REF_TO INT; c : Counter; sr : INT(0..10);
END_VAR
    f := 0;
    {stmt}
END_FUNCTION
"#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// The operation is checked as written out: a result the place cannot take
/// is E0301 on the statement, which says what it is and writes it out
/// converted; a literal of the wrong kind and an operator the type has not
/// are what `i := i + e` reports.
#[rstest]
fn invalid_an_operation_the_place_cannot_take(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR i : INT; d : DINT; s : STRING; on : BOOL; END_VAR
    i += d;
    i += 1.5;
    s += 'x';
    on += TRUE;
    f := i;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:5 ]
       |
     3 | VAR i : INT; d : DINT; s : STRING; on : BOOL; END_VAR
       |     |
       |     `-- 'i' is declared here
     4 |     i += d;
       |     ^^^|^^
       |        `---- expected 'INT', got 'DINT'
       |
       | Help: write 'i := DINT_TO_INT(i + d)'
       |
       | Note: 'i += d' is 'i := i + d'
    ---'
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:5:10 ]
       |
     5 |     i += 1.5;
       |     |    ^|^
       |     `--------- 'INT' is expected due to this
       |           |
       |           `--- invalid INT literal
       |
       | Note: INT takes a whole number, written like 42 or 16#2A
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:6:5 ]
       |
     3 | VAR i : INT; d : DINT; s : STRING; on : BOOL; END_VAR
       |                        |
       |                        `-- 's' is declared here
       |
     6 |     s += 'x';
       |     ^^^^|^^^
       |         `----- operator '+' cannot be applied to type 'STRING'
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:7:5 ]
       |
     3 | VAR i : INT; d : DINT; s : STRING; on : BOOL; END_VAR
       |                                    ^|
       |                                     `-- 'on' is declared here
       |
     7 |     on += TRUE;
       |     ^^^^^|^^^^
       |          `------ operator '+' cannot be applied to type 'BOOL'
    ---'
    ");
}

/// The place is checked as written, once: a CONSTANT, a name that is
/// nothing.
#[rstest]
fn invalid_a_place_that_cannot_be_written(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR CONSTANT K : INT := 1; END_VAR
    K += 1;
    nope += 1;
    f := K;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:4:5 ]
       |
     3 | VAR CONSTANT K : INT := 1; END_VAR
       |              |
       |              `-- 'K' is declared CONSTANT here
     4 |     K += 1;
       |     |
       |     `-- cannot write to constant 'K'
       |
       | Help: copy it into a variable to change the copy
       |
       | Note: a CONSTANT keeps the value it is declared with
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     nope += 1;
       |     ^^|^
       |       `--- no item 'nope' found in scope
    ---'
    ");
}

/// A compound assignment is a statement of its own: after a chain's place,
/// in an argument, after another compound's value or in a condition it is
/// E0033, a chain after its value is E0032, and with no value it is the
/// empty right-hand side. In a condition, the help says where the statement
/// goes: before an IF, before a WHILE and at the end of its body, on its
/// own for an ELSIF; and `/=` there most likely means "not equal", which is
/// the help, dividing the note.
#[rstest]
fn invalid_a_compound_assignment_outside_a_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION g : INT
VAR_INPUT a : INT; END_VAR
    g := a;
END_FUNCTION

FUNCTION f : INT
VAR a, b, x, y : INT; END_VAR
    a := b += 1;
    f := g(a += 1);
    x += y += 1;
    x += y := 1;
    x += ;
    IF x += 1 THEN f := 1; END_IF;
    IF y = 0 THEN f := 1; ELSIF y -= 2 THEN f := 2; END_IF;
    WHILE x -= 1 DO END_WHILE;
    REPEAT f := 2; UNTIL x /= 2 END_REPEAT;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0033] Error: compound assignment outside a statement
       ,-[ file:///test0.st:9:12 ]
       |
     9 |     a := b += 1;
       |            ^|
       |             `-- '+=' is a compound assignment
       |
       | Help: write 'b += 1;' in a statement before the chain
       |
       | Note: a compound assignment is a statement of its own
    ---'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     f := g(a += 1);
        |              ^|
        |               `-- '+=' is a compound assignment
        |
        | Help: write 'a += 1;' in a statement before the call
        |
        | Note: a compound assignment is a statement of its own
    ----'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:11:12 ]
        |
     11 |     x += y += 1;
        |            ^|
        |             `-- '+=' is a compound assignment
        |
        | Help: write 'y += 1;' in a statement before the '+='
        |
        | Note: a compound assignment is a statement of its own
    ----'
    [E0032] Error: chain of assignments outside a statement
        ,-[ file:///test0.st:12:12 ]
        |
     12 |     x += y := 1;
        |            ^|
        |             `-- a second ':=' chains an assignment
        |
        | Help: assign 'y' in a statement before the '+='
        |
        | Note: a chain of assignments is a statement of its own
    ----'
    [E0005] Error: empty right-hand side of assignment
        ,-[ file:///test0.st:13:7 ]
        |
     13 |     x += ;
        |       ^|
        |        `-- the assignment has no right-hand side
    ----'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:14:10 ]
        |
     14 |     IF x += 1 THEN f := 1; END_IF;
        |          ^|
        |           `-- '+=' is a compound assignment
        |
        | Help: write 'x += 1;' in a statement before the IF
        |
        | Note: a compound assignment is a statement of its own
    ----'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:15:35 ]
        |
     15 |     IF y = 0 THEN f := 1; ELSIF y -= 2 THEN f := 2; END_IF;
        |                                   ^|
        |                                    `-- '-=' is a compound assignment
        |
        | Help: write 'y -= 2;' in a statement of its own
        |
        | Note: a compound assignment is a statement of its own
    ----'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:16:13 ]
        |
     16 |     WHILE x -= 1 DO END_WHILE;
        |             ^|
        |              `-- '-=' is a compound assignment
        |
        | Help: write 'x -= 1;' in a statement before the WHILE and at the end of its body
        |
        | Note: a compound assignment is a statement of its own
    ----'
    [E0033] Error: compound assignment outside a statement
        ,-[ file:///test0.st:17:28 ]
        |
     17 |     REPEAT f := 2; UNTIL x /= 2 END_REPEAT;
        |                            ^|
        |                             `-- '/=' is a compound assignment
        |
        | Help: replace '/=' with '<>'
        |
        | Note 1: a compound assignment is a statement of its own
        |
        | Note 2: to divide, write 'x /= 2;' in a statement before the UNTIL
    ----'
    ");
}
