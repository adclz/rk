use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostic;
use crate::tests::utils::with_db;

#[rstest]
fn fuzzy_struct_fields(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                fuel1 : REAL;
                fuel2 : REAL;
                fuel3 : REAL;
                fuel4 : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                // fuel is not a member of engine
                Base : Engine := (power := 100, fuel := 10.0);
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
        ,-[ file:///test.st:15:49 ]
        |
      2 |         TYPE Engine:
        |              ^^^|^^  
        |                 `---- 'Engine' is declared here
        | 
     15 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^|^  
        |                                                   `--- No field 'fuel' in STRUCT
        | 
        | Note: STRUCT field(s) with similar name(s) exist:
        |       - fuel1
        |       - fuel2
        |       - fuel3
        |       - fuel4
    ----'
    ");
}

#[rstest]
fn fuzzy_local_variables(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
            VAR
                engine2: INT;
                no_engine: INT;
                oil: INT;
            END_VAR

            engine := ULINT#5;

        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:9:13 ]
       |
     9 |             engine := ULINT#5;
       |             ^^^|^^  
       |                `---- no item 'engine' in scope
       | 
       | Note: local variable(s) with similar(s) name exist:
       |       - engine2
       |       - no_engine
    ---'
    ");
}
