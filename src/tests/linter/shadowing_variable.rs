use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn variable_shadows_function_block(mut with_db: RootDatabase) {
    let source_fb = r#"
        FUNCTION_BLOCK PrintLog
        END_FUNCTION_BLOCK
    "#;
    let source_fn = r#"
        FUNCTION test : INT
        VAR
            PrintLog : BOOL;
        END_VAR
            PrintLog := TRUE;
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source_fb, source_fn]), @r"
    [W0102] Error: name shadowing
       ,-[ file:///test1.st:4:13 ]
       |
     4 |             PrintLog : BOOL;
       |             ^^^^^^^|^^^^^^^  
       |                    `--------- variable 'PrintLog' shadows POU 'PrintLog' available in this scope
       |
       |-[ file:///test0.st:2:24 ]
       |
     2 |         FUNCTION_BLOCK PrintLog
       |                        ^^^^|^^^  
       |                            `----- POU PrintLog is declared here
    ---'
    ");
}

#[rstest]
fn variable_shadows_function(mut with_db: RootDatabase) {
    let source_fn1 = r#"
        FUNCTION helper : INT
            helper := 0;
        END_FUNCTION
    "#;
    let source_fn2 = r#"
        FUNCTION test : INT
        VAR
            helper : INT;
        END_VAR
            helper := 1;
            test := helper;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source_fn1, source_fn2]), @r"
    [W0102] Error: name shadowing
       ,-[ file:///test1.st:4:13 ]
       |
     4 |             helper : INT;
       |             ^^^^^^|^^^^^  
       |                   `------- variable 'helper' shadows POU 'helper' available in this scope
       |
       |-[ file:///test0.st:2:18 ]
       |
     2 |         FUNCTION helper : INT
       |                  ^^^|^^  
       |                     `---- POU helper is declared here
    ---'
    ");
}

#[rstest]
fn variable_shadows_data_type(mut with_db: RootDatabase) {
    let source_type = r#"
        TYPE MyType : STRUCT
            x : INT;
        END_STRUCT;
        END_TYPE
    "#;
    let source_fn = r#"
        FUNCTION test : INT
        VAR
            MyType : INT;
        END_VAR
            MyType := 1;
            test := MyType;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source_type, source_fn]), @r"
    [W0102] Error: name shadowing
       ,-[ file:///test1.st:4:13 ]
       |
     4 |             MyType : INT;
       |             ^^^^^^|^^^^^  
       |                   `------- variable 'MyType' shadows POU 'MyType' available in this scope
       |
       |-[ file:///test0.st:2:14 ]
       |
     2 |         TYPE MyType : STRUCT
       |              ^^^|^^  
       |                 `---- POU MyType is declared here
    ---'
    ");
}

#[rstest]
fn no_shadowing_when_names_differ(mut with_db: RootDatabase) {
    let source_fb = r#"
        FUNCTION_BLOCK Logger
        END_FUNCTION_BLOCK
    "#;
    let source_fn = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x := 1;
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source_fb, source_fn]), @r"");
}

#[rstest]
fn no_shadowing_when_variable_unused(mut with_db: RootDatabase) {
    // Variable declared with same name as POU but never used in body.
    // Shadowing is usage-based, so only W0101 (unused) is emitted.
    let source_fb = r#"
        FUNCTION_BLOCK PrintLog
        END_FUNCTION_BLOCK
    "#;
    let source_fn = r#"
        FUNCTION test : INT
        VAR
            PrintLog : BOOL;
        END_VAR
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source_fb, source_fn]), @r"
    [W0101] Warning: unused code
       ,-[ file:///test1.st:4:13 ]
       |
     4 |             PrintLog : BOOL;
       |             ^^^^^^^|^^^^^^^  
       |                    `--------- unused variable 'PrintLog'
       | 
       | Note: if this is intentional, prefix it with an underscore:
       |       '_PrintLog'
    ---'
    ");
}
