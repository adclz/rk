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
    Error: 
       ,-[ file:///test0.st:6:2 ]
       |
     6 |     THIS.decl1();
       |     ^^^^^^|^^^^^  
       |           `------- no method 'decl1' in declared methods of 'fb1'
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
    Error: 
       ,-[ file:///test0.st:8:5 ]
       |
     2 | CLASS base
       |       ^^|^  
       |         `--- methods are inherited from 'base' here
       | 
     8 |     SUPER.super_method1()
       |     ^^^^^^^^^^|^^^^^^^^^^  
       |               `------------ no method 'super_method1' in inherited methods
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
    Error: 
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         SUPER()
       |         ^^^|^^^  
       |            `----- SUPER() can only be called in FUNCTION_BLOCK POUs
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
    Error: 
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     SUPER()
       |     ^^^|^^^  
       |        `----- SUPER() can only be called in FUNCTION_BLOCK POUs
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
    Error: 
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     SUPER.something()
       |     ^^^^^^^^|^^^^^^^^  
       |             `---------- SUPER can only be used in FUNCTION_BLOCK or CLASS POUs
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
    Error: 
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     THIS.something()
       |     ^^^^^^^^|^^^^^^^  
       |             `--------- THIS can only be used in in FUNCTION_BLOCK or CLASS POUs
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
        VAR_INPUT input1 : INT; END_VAR
	END_METHOD

	THIS.decl(0.5);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:12 ]
       |
     4 |         VAR_INPUT input1 : INT; END_VAR
       |                            ^|^  
       |                             `--- expected type 'INT' here
       | 
     7 |     THIS.decl(0.5);
       |               ^|^  
       |                `--- parameter expression mismatch: invalid INT literal
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
        VAR_INPUT input1 : INT; END_VAR
	END_METHOD
END_CLASS

CLASS derived EXTENDS base
	METHOD decl2
	    SUPER.decl(0.5);
	END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : INT; END_VAR
        |                            ^|^  
        |                             `--- expected type 'INT' here
        | 
     10 |         SUPER.decl(0.5);
        |                    ^|^  
        |                     `--- parameter expression mismatch: invalid INT literal
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
        VAR_INPUT input1 : INT; END_VAR
	END_METHOD
END_CLASS

FUNCTION_BLOCK derived EXTENDS base
	 METHOD decl2
	    SUPER.decl(0.5);
	 END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:10:17 ]
        |
      4 |         VAR_INPUT input1 : INT; END_VAR
        |                            ^|^  
        |                             `--- expected type 'INT' here
        | 
     10 |         SUPER.decl(0.5);
        |                    ^|^  
        |                     `--- parameter expression mismatch: invalid INT literal
    ----'
    ");
}
