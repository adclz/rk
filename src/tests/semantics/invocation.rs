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
    [E0211] Error: resolution failure
       ,-[ file:///test0.st:6:7 ]
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
    [E0211] Error: resolution failure
       ,-[ file:///test0.st:8:11 ]
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
    [E0501] Error: inheritance violation
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
    [E0501] Error: inheritance violation
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
    [E0502] Error: inheritance violation
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
    [E0503] Error: inheritance violation
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
    [E0309] Error: type mismatch
       ,-[ file:///test0.st:7:12 ]
       |
     4 |         VAR_INPUT input1 : BOOL; END_VAR
       |                   ^^^^^^|^^^^^^  
       |                         `-------- 'BOOL' is expected due to this
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
    [E0309] Error: type mismatch
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : BOOL; END_VAR
        |                   ^^^^^^|^^^^^^  
        |                         `-------- 'BOOL' is expected due to this
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
    [E0309] Error: type mismatch
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : BOOL; END_VAR
        |                   ^^^^^^|^^^^^^  
        |                         `-------- 'BOOL' is expected due to this
        | 
     10 |         SUPER.decl(0.5);
        |                    ^|^  
        |                     `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ----'
    ");
}
