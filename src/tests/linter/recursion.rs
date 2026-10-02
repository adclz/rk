// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A call leading back to its caller is warned where it is written, with the
/// way back: a call to itself, a cycle through another FUNCTION.
#[rstest]
fn recursion_rendering(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SumTo : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN SumTo := n + SumTo(n - 1); END_IF;
END_FUNCTION

FUNCTION Ping : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN Ping := Pong(n - 1); END_IF;
END_FUNCTION

FUNCTION Pong : INT
VAR_INPUT n : INT; END_VAR
    Pong := Ping(n);
END_FUNCTION

FUNCTION Caller : INT
    Caller := SumTo(3) + Ping(2);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "recursion"), @r"
    [L0122] Warning: recursive call
       ,-[ file:///test0.st:4:32 ]
       |
     4 |     IF n > 0 THEN SumTo := n + SumTo(n - 1); END_IF;
       |                                ^^|^^
       |                                  `---- 'SumTo' calls itself
       |
       | Note 1: each call gets its own frame on the stack, and too deep a recursion stops the program
       |
       | Note 2: lint rule: recursion
    ---'
    [L0122] Warning: recursive call
       ,-[ file:///test0.st:9:27 ]
       |
     9 |     IF n > 0 THEN Ping := Pong(n - 1); END_IF;
       |                           ^^|^
       |                             `--- 'Ping' calls itself through 'Pong'
       |
       | Note 1: each call gets its own frame on the stack, and too deep a recursion stops the program
       |
       | Note 2: lint rule: recursion
    ---'
    [L0122] Warning: recursive call
        ,-[ file:///test0.st:14:13 ]
        |
     14 |     Pong := Ping(n);
        |             ^^|^
        |               `--- 'Pong' calls itself through 'Ping'
        |
        | Note 1: each call gets its own frame on the stack, and too deep a recursion stops the program
        |
        | Note 2: lint rule: recursion
    ----'
    ");
}

/// Only the calls that stay on the cycle are warned: `Leaf()` returns.
#[rstest]
fn only_the_calls_on_the_cycle_are_warned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Leaf : INT
    Leaf := 1;
END_FUNCTION

FUNCTION Walk : INT
VAR_INPUT n : INT; END_VAR
    Walk := Leaf();
    IF n > 0 THEN Walk := Walk(n - 1); END_IF;
END_FUNCTION
"#;
    let rendered = test_single_lint(&mut with_db, &[source], "recursion");
    assert_eq!(rendered.matches("[L0122]").count(), 1, "{rendered}");
    assert!(rendered.contains("'Walk' calls itself"), "{rendered}");
}

/// Through `THIS`, a base method runs the override that calls it back.
#[rstest]
fn recursion_through_an_override(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
    METHOD PUBLIC Run : INT
        Run := THIS.Hook();
    END_METHOD
    METHOD PUBLIC Hook : INT
        Hook := 0;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Hook : INT
        Hook := Run();
    END_METHOD
END_FUNCTION_BLOCK
"#;
    let rendered = test_single_lint(&mut with_db, &[source], "recursion");
    assert!(
        rendered.contains("'Base.Run' calls itself through 'Derived.Hook'"),
        "{rendered}"
    );
    assert!(
        rendered.contains("'Derived.Hook' calls itself through 'Base.Run'"),
        "{rendered}"
    );
}

/// Nothing without a cycle, and the warning can be allowed like any other.
#[rstest]
fn no_cycle_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION A : INT
    A := B() + C();
END_FUNCTION

FUNCTION B : INT
    B := C();
END_FUNCTION

FUNCTION C : INT
    C := 1;
END_FUNCTION

{allow 'recursion'}
FUNCTION Fact : DINT
VAR_INPUT n : DINT; END_VAR
    Fact := 1;
    IF n > 1 THEN Fact := n * Fact(n - 1); END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "recursion"), @"");
}
