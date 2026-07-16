use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn unresolved_this_method_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
	METHOD decl
	END_METHOD

	THIS.decl1();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
       ,-[ file:///test0.st:6:7 ]
       |
     2 | FUNCTION_BLOCK fb1
       |                ^|^
       |                 `--- FUNCTION_BLOCK 'fb1' is defined here
       |
     6 |     THIS.decl1();
       |          ^^|^^
       |            `---- 'fb1' has no field named 'decl1'
    ---'
    ");
}

// Valid case
#[rstest]
fn resolved_this_method_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
	METHOD decl
	END_METHOD

	THIS.decl();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn unresolved_super_method_in_fb(mut with_db: RootDatabase) {
    let source = r#"
CLASS base
	METHOD PUBLIC super_method END_METHOD
END_CLASS

FUNCTION_BLOCK fb1 EXTENDS base

    SUPER.super_method1()

END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
       ,-[ file:///test0.st:8:11 ]
       |
     6 | FUNCTION_BLOCK fb1 EXTENDS base
       |                ^|^
       |                 `--- FUNCTION_BLOCK 'fb1' is defined here
       |
     8 |     SUPER.super_method1()
       |           ^^^^^^|^^^^^^
       |                 `-------- 'fb1' has no field named 'super_method1'
    ---'
    ");
}

#[rstest]
fn super_keyword_body_in_class(mut with_db: RootDatabase) {
    let source = r#"
CLASS fb1
    METHOD method1
        SUPER()
    END_METHOD
END_CLASS"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0501] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         SUPER()
       |         ^^|^^
       |           `---- 'SUPER()' is not valid in this context
    ---'
    ");
}

#[rstest]
fn super_keyword_body_in_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    SUPER()
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0501] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     SUPER()
       |     ^^|^^
       |       `---- 'SUPER()' is not valid in this context
    ---'
    ");
}

#[rstest]
fn super_keyword_in_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    SUPER.something()
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0502] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     SUPER.something()
       |     ^^|^^
       |       `---- 'SUPER' is not valid in this context
    ---'
    ");
}

#[rstest]
fn this_keyword_in_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    THIS.something()
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     THIS.something()
       |     ^^|^
       |       `--- 'THIS' is not valid in this context
    ---'
    ");
}

// The logic for checking invocations is the same as function calls.
// So we just test if parameters are passed correctly.

#[rstest]
fn type_check_this_method_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
	METHOD decl
        VAR_INPUT input1 : BOOL; END_VAR
	END_METHOD

	THIS.decl(0.5);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:7:12 ]
       |
     4 |         VAR_INPUT input1 : BOOL; END_VAR
       |                   ^^^|^^
       |                      `---- type is declared by variable 'input1' here
       |
     7 |     THIS.decl(0.5);
       |               ^|^
       |                `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ---'
    ");
}

// The logic for checking invocations is the same as function calls.
// So we just test if parameters are passed correctly.
#[rstest]
fn type_check_super_method_in_class_method(mut with_db: RootDatabase) {
    let source = r#"
CLASS base
	METHOD PUBLIC decl
        VAR_INPUT input1 : BOOL; END_VAR
	END_METHOD
END_CLASS

CLASS derived EXTENDS base
	METHOD decl2
	    SUPER.decl(0.5);
	END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : BOOL; END_VAR
        |                   ^^^|^^
        |                      `---- type is declared by variable 'input1' here
        |
     10 |         SUPER.decl(0.5);
        |                    ^|^
        |                     `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ----'
    ");
}

// The logic for checking invocations is the same as function calls.
// So we just test if parameters are passed correctly.
#[rstest]
fn type_check_super_method_in_fb_method(mut with_db: RootDatabase) {
    let source = r#"
CLASS base
	METHOD PUBLIC decl
        VAR_INPUT input1 : BOOL; END_VAR
	END_METHOD
END_CLASS

FUNCTION_BLOCK derived EXTENDS base
	 METHOD decl2
	    SUPER.decl(0.5);
	 END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : BOOL; END_VAR
        |                   ^^^|^^
        |                      `---- type is declared by variable 'input1' here
        |
     10 |         SUPER.decl(0.5);
        |                    ^|^
        |                     `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ----'
    ");
}

#[rstest]
fn super_body_no_extends(mut with_db: RootDatabase) {
    // SUPER() (base-body call) in an FB with no EXTENDS -> E0513: there is no
    // base function block whose body could be executed.
    let source = r#"
FUNCTION_BLOCK fb1
    SUPER()
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0513] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     SUPER()
       |     ^^|^^
       |       `---- 'SUPER' used but no EXTENDS clause found on 'fb1'
    ---'
    ");
}

#[rstest]
fn super_body_in_method(mut with_db: RootDatabase) {
    // Rule 5: SUPER() (base-body call) in a METHOD is forbidden, even when the FB
    // extends a base (so E0513 does not apply) -> E0518.
    let source = r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK
FUNCTION_BLOCK derived EXTENDS base
    METHOD m1
        SUPER()
    END_METHOD
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0518] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:6:9 ]
       |
     6 |         SUPER()
       |         ^^|^^
       |           `---- 'SUPER()' cannot be called in a method of a function block
       |
       | Note: SUPER() is only valid in the function block body, not in a method
    ---'
    ");
}

#[rstest]
fn super_body_multiple(mut with_db: RootDatabase) {
    // Rule 2: SUPER() shall occur once. A second SUPER() -> E0519, with related
    // info pointing at the first.
    let source = r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK
FUNCTION_BLOCK derived EXTENDS base
    SUPER();
    SUPER();
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0519] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:6:5 ]
       |
     5 |     SUPER();
       |     ^^^|^^^
       |        `----- 'SUPER()' is already called here
     6 |     SUPER();
       |     ^^^|^^^
       |        `----- 'SUPER()' may only be called once in a function block body
    ---'
    ");
}

#[rstest]
fn super_body_in_loop(mut with_db: RootDatabase) {
    // Rule 2: SUPER() shall not be in a loop -> E0520.
    let source = r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK
FUNCTION_BLOCK derived EXTENDS base
    VAR i : INT; END_VAR
    FOR i := 1 TO 3 DO
        SUPER();
    END_FOR
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0520] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:7:9 ]
       |
     7 |         SUPER();
       |         ^^^|^^^
       |            `----- 'SUPER()' cannot be called inside a loop
    ---'
    ");
}

#[rstest]
fn super_body_valid(mut with_db: RootDatabase) {
    // Valid: exactly one SUPER(), in the FB body (not a loop, not a method),
    // in an FB that extends a base. No diagnostics. An IF (not a loop) is fine.
    let source = r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK
FUNCTION_BLOCK derived EXTENDS base
    VAR flag : BOOL; END_VAR
    IF flag THEN
        SUPER();
    END_IF
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
