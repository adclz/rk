use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn missing_override(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // missing override keyword
            METHOD Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:20 ]
       |
     3 |             METHOD Tick : INT END_METHOD
       |                    ^^|^  
       |                      `--- 'Tick' is declared here
       | 
     8 |             METHOD Tick : INT END_METHOD
       |                    ^^|^  
       |                      `--- missing OVERRIDE keyword for method 'Tick'
       | 
       | Note: OVERRIDE keyword must be used even if the base method is not marked as ABSTRACT
    ---'
    ");
}

#[rstest]
fn override_final_method(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD FINAL Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // override of FINAL method
            METHOD OVERRIDE Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:29 ]
       |
     3 |             METHOD FINAL Tick : INT END_METHOD
       |                          ^^|^  
       |                            `--- 'Tick' is declared here
       | 
     8 |             METHOD OVERRIDE Tick : INT END_METHOD
       |                             ^^|^  
       |                               `--- cannot override FINAL method 'Tick'
       | 
       | Note: methods marked as FINAL cannot be overridden
    ---'
    ");
}

#[rstest]
fn missing_abstract_method(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD ABSTRACT Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // missing implementation of method
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD ABSTRACT Tick : INT END_METHOD
       |                             ^^|^  
       |                               `--- 'Tick' is declared here
       | 
     6 |         CLASS Mid EXTENDS Base
       |               ^|^  
       |                `--- missing implementation for ABSTRACT method 'Tick'
       | 
       | Note: ABSTRACT methods must be implemented by derived POUs
    ---'
    ");
}

#[rstest]
fn empty_override(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD OVERRIDE Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             METHOD OVERRIDE Tick : INT END_METHOD
       |                             ^^|^  
       |                               `--- invalid usage of OVERRIDE for method 'Tick'
       | 
       | Note: OVERRIDE is only valid when the method is inherited
    ---'
    ");
}

#[rstest]
fn abstract_class_has_no_abstract_methods(mut with_db: RootDatabase) {
    let source = r#"
        CLASS ABSTRACT Base
            METHOD Tick : INT END_METHOD
            METHOD Tick2 : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:24 ]
       |
     2 |         CLASS ABSTRACT Base
       |                        ^^|^  
       |                          `--- ABSTRACT class 'Base' has no abstract methods
       | 
       | Note: abstract classes must have at least one abstract method
    ---'
    ");
}

#[rstest]
fn interface_methods_not_implemented(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD DAYTIME END_METHOD
       |                    ^^^|^^^  
       |                       `----- 'DAYTIME' is declared here
       | 
     6 |         CLASS Mid IMPLEMENTS ROOM1
       |               ^|^  
       |                `--- missing implementation for interface method 'DAYTIME'
    ---'
    ");
}
