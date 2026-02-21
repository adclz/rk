use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^  
       |           `--- type is declared by variable 'test' here
       | 
     7 |     test := ULINT#5;
       |             ^^^|^^^  
       |                `----- expected 'INT', got 'ULINT'
       |                |     
       |                `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       | 
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1003] Error: assignment violation
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     fb2 := ULINT#5;
       |     ^|^  
       |      `--- cannot use direct type 'fb2' here
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:12 ]
       |
     2 | FUNCTION_BLOCK fb2
       |                ^|^  
       |                 `--- FUNCTION_BLOCK 'fb2' is defined here
       | 
     8 |     fb2 := ULINT#5;
       |            ^^^|^^^  
       |               `----- expected 'fb2', got 'ULINT'
    ---'
    ");
}

#[rstest]
fn assign_function_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT

    fn1 := ULINT#5;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:12 ]
       |
     2 | FUNCTION fn1 : INT
       |          ^|^  
       |           `--- FUNCTION 'fn1' is defined here, with return type 'INT'
       | 
     4 |     fn1 := ULINT#5;
       |            ^^^|^^^  
       |               `----- expected 'INT', got 'ULINT'
       |               |     
       |               `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       | 
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
    ---'
    ");
}

#[rstest]
fn assign_function_with_no_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1

    fn1 := ULINT#5;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:12 ]
       |
     2 | FUNCTION fn1
       |          ^|^  
       |           `--- FUNCTION 'fn1' is defined here
       | 
     4 |     fn1 := ULINT#5;
       |            ^^^|^^^  
       |               `----- 'fn1' is void and can not be assigned
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1003] Error: assignment violation
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     T1 := ULINT#5;
       |     ^|  
       |      `-- cannot use direct type 'T1' here
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     3 |     T1 : INT;
       |          ^|^  
       |           `--- type is defined by 'T1' here
       | 
     8 |     T1 := ULINT#5;
       |           ^^^|^^^  
       |              `----- expected 'T1', got 'ULINT'
       |              |     
       |              `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       | 
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1001] Error: assignment violation
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     d_fb2 := ULINT#5;
        |     ^^|^^  
        |       `---- 'fb2' is a callable type and can not be assigned
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:14 ]
        |
      2 | FUNCTION_BLOCK fb2
        |                ^|^  
        |                 `--- FUNCTION_BLOCK 'fb2' is defined here
        | 
     11 |     d_fb2 := ULINT#5;
        |              ^^^|^^^  
        |                 `----- expected 'fb2', got 'ULINT'
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1002] Error: assignment violation
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     test := 5;
       |     ^^|^  
       |       `--- test is an input variable and can not be assigned
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:13 ]
        |
      8 |         test: INT;
        |         ^^|^  
        |           `--- type is declared by variable 'test' here
        | 
     11 |     test := fn1();
        |             ^^|^^  
        |               `---- expected 'INT', got 'void'
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:13 ]
        |
      8 |         test: INT;
        |         ^^|^  
        |           `--- type is declared by variable 'test' here
        | 
     11 |     test := fn1();
        |             ^^|^^  
        |               `---- expected 'INT', got 'BOOL'
        |               |    
        |               `---- consider explicitly casting with 'BOOL_TO_INT(fn1())'
        | 
        | Help: insert explicit cast 'INT_TO_BOOL(fn1())'
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^  
       |           `--- type is declared by variable 'test' here
       | 
     7 |     test := TRUE AND FALSE;
       |             ^^^^^^^|^^^^^^  
       |                    `-------- expected 'INT', got 'BOOL'
       |                    |        
       |                    `-------- consider explicitly casting with 'BOOL_TO_INT(TRUE AND FALSE)'
       | 
       | Help: insert explicit cast 'INT_TO_BOOL(TRUE AND FALSE)'
    ---'
    ");
}

#[rstest]
fn assign_parenthesized_comparison_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: INT := 5;
        b: INT := 10;
        result: BOOL;
    END_VAR

    // Parenthesized comparison should return BOOL
    result := (a < b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_parenthesized_boolean_op_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: BOOL := TRUE;
        b: BOOL := FALSE;
        result: BOOL;
    END_VAR

    // Parenthesized boolean operator should work
    result := (a AND b);
    result := NOT (a OR b);
    result := (a >= b) AND (a <= b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_parenthesized_comparison_to_int_invalid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: INT := 5;
        b: INT := 10;
        result: INT;
    END_VAR

    // Parenthesized comparison returns BOOL, not INT
    result := (a < b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:15 ]
        |
      6 |         result: INT;
        |         ^^^|^^  
        |            `---- type is declared by variable 'result' here
        | 
     10 |     result := (a < b);
        |               ^^^|^^^  
        |                  `----- expected 'INT', got 'BOOL'
        |                  |     
        |                  `----- consider explicitly casting with 'BOOL_TO_INT((a < b))'
        | 
        | Help: insert explicit cast 'INT_TO_BOOL((a < b))'
    ----'
    ");
}
