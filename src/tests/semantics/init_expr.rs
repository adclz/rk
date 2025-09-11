use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostic;
use crate::tests::utils::with_db;

#[rstest]
fn mismatch_field_name_in_struct(mut with_db: RootDatabase) {
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
        | Note: Did you mean:
        |       - fuel1
        |       - fuel2
        |       - fuel3
        |       - fuel4
    ----'
    ");
}
