use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
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
fn duplicate_enum_variants(mut with_db: RootDatabase) {
    let source = r#"
TYPE 
    E1 : (A, B, A);
END_TYPE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:11 ]
       |
     3 |     E1 : (A, B, A);
       |           |     |  
       |           `-------- duplicate enum variant 'A'
       |                 |  
       |                 `-- enum variant 'A' is already defined here
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

// DashMap keys are not ordered
// This means the snapshot might not be in the order we expect.
// So instead we just check the number of errors

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

    let source3 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source1, source2, source3]);

    let diagnostics = with_db
        .get_files()
        .iter()
        .map(|file| diagnostics_for_file(&with_db, *file))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 3);
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

    add_sources(&mut with_db, &[source1, source2]);

    let diagnostics = with_db
        .get_files()
        .iter()
        .map(|file| diagnostics_for_file(&with_db, *file))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
}

#[rstest]
fn duplicate_methods_in_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_INTERFACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- duplicate method 'm1'
     4 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- method 'm1' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_class(mut with_db: RootDatabase) {
    let source = r#"
CLASS it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- duplicate method 'm1'
     4 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- method 'm1' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- duplicate method 'm1'
     4 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- method 'm1' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_inherited_methods(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE I1
    METHOD m1 END_METHOD
END_INTERFACE

INTERFACE I2
    METHOD m1 END_METHOD
END_INTERFACE

CLASS it1 IMPLEMENTS I1, I2
    METHOD OVERRIDE m1 END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- duplicate method 'm1'
       | 
     7 |     METHOD m1 END_METHOD
       |            ^|  
       |             `-- method 'm1' is already defined here
       | 
       | Note: this error happens because both interfaces 'I1' and 'I2' define a method 'm1'
    ---'
    ");
}
