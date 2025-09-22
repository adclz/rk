use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn duplicate_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
        test: REAL;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         test: INT;
       |         ^^|^  
       |           `--- variable 'test' is already defined here
     5 |         test: REAL;
       |         ^^|^  
       |           `--- duplicate variable 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_inline_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test, test: INT;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:15 ]
       |
     4 |         test, test: INT;
       |         ^^|^  ^^|^  
       |           `--------- variable 'test' is already defined here
       |                 |   
       |                 `--- duplicate variable 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_struct_fields(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    T1 : STRUCT
        test: INT;
        test: REAL;
    END_STRUCT;
END_TYPE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         test: INT;
       |         ^^|^  
       |           `--- field 'test' is already defined here
     5 |         test: REAL;
       |         ^^|^  
       |           `--- duplicate field 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_pous(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:6:16 ]
       |
     2 | FUNCTION_BLOCK fb1
       |                ^|^  
       |                 `--- POU 'fb1' is already defined here
       | 
     6 | FUNCTION_BLOCK fb1
       |                ^|^  
       |                 `--- duplicate POU 'fb1'
    ---'
    ");
}

#[rstest]
fn duplicate_pous_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- POU 'fb1' is already defined here
       | 
     7 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- duplicate POU 'fb1'
    ---'
    ");
}

#[rstest]
fn cross_file_global_duplicates(mut with_db: RootDatabase) {
    let source1 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
"#;

    let source2 = r#"

    FUNCTION_BLOCK fb1
    
    END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source1, source2]), @r"
    Error: 
       ,-[ file:///test0.st:2:20 ]
       |
     2 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- duplicate POU 'fb1'
       |
       |-[ file:///test1.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- POU 'fb1' is already defined here
    ---'
    ");
}



#[rstest]
fn cross_file_namespace_duplicates(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1
    
    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source1, source2]), @r"
    Error: 
       ,-[ file:///test0.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- duplicate POU 'fb1'
       |
       |-[ file:///test1.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- POU 'fb1' is already defined here
    ---'
    Error: 
       ,-[ file:///test1.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- duplicate POU 'fb1'
       |
       |-[ file:///test0.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^  
       |                     `--- POU 'fb1' is already defined here
    ---'
    ");
}

