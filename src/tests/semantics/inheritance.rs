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
    [E0503] Error: override violation
       ,-[ file:///test0.st:8:20 ]
       |
     3 |             METHOD Tick : INT END_METHOD
       |                    ^^|^  
       |                      `--- base method 'Tick' is declared here
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
    [E0502] Error: override violation
       ,-[ file:///test0.st:8:29 ]
       |
     3 |             METHOD FINAL Tick : INT END_METHOD
       |                          ^^|^  
       |                            `--- FINAL method 'Tick' is declared here
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
    [E0504] Error: inheritance violation
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD ABSTRACT Tick : INT END_METHOD
       |                             ^^|^  
       |                               `--- ABSTRACT method 'Tick' is declared here
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
    [E0505] Error: inheritance violation
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
    [E0506] Error: inheritance violation
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
    [E0507] Error: inheritance violation
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD DAYTIME END_METHOD
       |                    ^^^|^^^  
       |                       `----- method 'DAYTIME' is declared by interface 'Mid' here
       | 
     6 |         CLASS Mid IMPLEMENTS ROOM1
       |               ^|^  
       |                `--- missing implementation for interface method 'DAYTIME'
    ---'
    ");
}

#[rstest]
fn method_signature_count_mismatch_in_implementer(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
                VAR_INPUT
                    value: INT
                END_VAR
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME

            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0510] Error: method signature mismatch
        ,-[ file:///test0.st:11:29 ]
        |
      3 |             METHOD DAYTIME
        |                    ^^^|^^^  
        |                       `----- base method 'DAYTIME' is declared here
        | 
     11 |             METHOD OVERRIDE DAYTIME
        |                             ^^^|^^^  
        |                                `----- invalid number of parameters for method 'DAYTIME': expected 1, got 0
    ----'
    ");
}

#[rstest]
fn method_signature_count_mismatch_in_base(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME
                VAR_INPUT
                    value: INT
                END_VAR
            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0510] Error: method signature mismatch
       ,-[ file:///test0.st:8:29 ]
       |
     3 |             METHOD DAYTIME
       |                    ^^^|^^^  
       |                       `----- base method 'DAYTIME' is declared here
       | 
     8 |             METHOD OVERRIDE DAYTIME
       |                             ^^^|^^^  
       |                                `----- invalid number of parameters for method 'DAYTIME': expected 0, got 1
    ---'
    ");
}

#[rstest]
fn method_signature_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
                VAR_INPUT
                    value: INT;
                    value2: INT;
                END_VAR
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME
                VAR_INPUT
                    value: INT;
                    value2: REAL; // should be INT
                END_VAR
            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0511] Error: method signature mismatch
        ,-[ file:///test0.st:12:29 ]
        |
     12 |             METHOD OVERRIDE DAYTIME
        |                             ^^^|^^^  
        |                                `----- method 'DAYTIME' has incompatible parameter types: expected 'INT', got 'REAL'
        | 
        | Note: parameter types must match those of the base method
    ----'
    ");
}

#[rstest]
fn super_without_extends_clause_on_fb(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
            SUPER.something
        END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             SUPER.something
       |             ^^|^^  
       |               `---- 'SUPER' used but no EXTENDS clause found on 'fb'
    ---'
    ");
}

#[rstest]
fn super_without_extends_clause_on_class(mut with_db: RootDatabase) {
    let source = r#"
        CLASS class
           METHOD doSomething
                SUPER.something
           END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:4:17 ]
       |
     4 |                 SUPER.something
       |                 ^^|^^  
       |                   `---- 'SUPER' used but no EXTENDS clause found on 'class'
    ---'
    ");
}
