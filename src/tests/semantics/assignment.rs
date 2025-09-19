use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostic;
use crate::tests::utils::with_db;

#[rstest]
fn assign_mismatch_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^  ^|^  
       |           `-------- 'test' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     test := ULINT#5;
       |             ^^^|^^^  
       |                `----- invalid INT literal
       | 
       | Note: An INT literal must be an integer between -32768 and 32767
    ---'
    ");
}

#[rstest]
fn assign_undeclared_pou_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1

    fb2 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:8:5 ]
       |
     2 | ,-> FUNCTION_BLOCK fb2
       | |                  ^|^  
       | |                   `--- 'fb2' is declared here
       : :   
     4 | |-> END_FUNCTION_BLOCK
       | |                        
       | `------------------------ type defined here
       | 
     8 |         fb2 := ULINT#5;
       |         ^|^  
       |          `--- 'fb2' is a type and can not be assigned
       | |   
       | |   Note: types can only be assigned if they are declared in a VAR_* section
    ---'
    ");
}

#[rstest]
fn assign_undeclared_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    T1 : INT;
END_TYPE

FUNCTION_BLOCK fb1

    T1 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:8:5 ]
       |
     3 |     T1 : INT;
       |     ^|   ^|^  
       |      `-------- 'T1' is declared here
       |           |   
       |           `--- type defined here
       | 
     8 |     T1 := ULINT#5;
       |     ^|  
       |      `-- 'T1' is a type and can not be assigned
       | 
       | Note: types can only be assigned if they are declared in a VAR_* section
    ---'
    ");
}

#[rstest]
fn assign_indirect_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        d_fb2: fb2;
    END_VAR

    d_fb2 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
        ,-[ file:///test.st:11:5 ]
        |
      2 | ,-> FUNCTION_BLOCK fb2
        : :   
      4 | |-> END_FUNCTION_BLOCK
        | |                        
        | `------------------------ type defined here
        | 
      8 |             d_fb2: fb2;
        |             ^^|^^  
        |               `---- 'd_fb2' is declared here
        | 
     11 |         d_fb2 := ULINT#5;
        |         ^^|^^  
        |           `---- 'd_fb2' is a callable type and can not be assigned
    ----'
    ");
}

#[rstest]
fn assign_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_INPUT
        test: INT;
    END_VAR

    test := 5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Warning: 
       ,-[ file:///test.st:7:5 ]
       |
     4 |         test: INT;
       |         ^^|^  
       |           `--- 'test' is declared here
       | 
     7 |     test := 5;
       |     ^^|^  
       |       `--- 'test' is an input variable and should not be assigned
    ---'
    ");
}

#[rstest]
fn assign_void_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := fn1();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
        ,-[ file:///test.st:11:13 ]
        |
      8 |         test: INT;
        |         ^^|^  
        |           `--- 'test' is declared here
        | 
     11 |     test := fn1();
        |             ^|^  
        |              `--- target is of type void
    ----'
    ");
}

#[rstest]
fn assign_invalid_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : BOOL

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := fn1();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
        ,-[ file:///test.st:11:13 ]
        |
      2 | FUNCTION fn1 : BOOL
        |          ^|^   ^^|^  
        |           `---------- 'fn1' is declared here
        |                  |   
        |                  `--- type defined here
        | 
     11 |     test := fn1();
        |             ^^|^^  
        |               `---- type mismatch: 'test' and 'fn1'
    ----'
    ");
}

#[rstest]
fn assign_compare_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := TRUE AND FALSE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^  ^|^  
       |           `-------- 'test' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     test := TRUE AND FALSE;
       |             ^^^^^^^|^^^^^^  
       |                    `-------- a boolean expression can not be assigned because 'test' is not a boolean
    ---'
    ");
}
