use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostic;
use crate::tests::utils::with_db;

#[rstest]
fn unknown_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
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
        ,-[ file:///test.st:12:49 ]
        |
      2 |         TYPE Engine:
        |              ^^^|^^  
        |                 `---- 'Engine' is declared here
        | 
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^|^  
        |                                                   `--- No field 'fuel' in STRUCT
    ----'
    ");
}
