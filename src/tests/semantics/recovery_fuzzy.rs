use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:15:49 ]
        |
      2 |         TYPE Engine:
        |              ^^^|^^
        |                 `---- type 'Engine: STRUCT' defined here
        |
     15 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^|^
        |                                                   `--- no field 'fuel' in STRUCT
        |
        | Note: STRUCT has fields with similar name:
        |       - fuel1
        |       - fuel2
        |       - fuel3
        |       - fuel4
    ----'
    ");
}

#[rstest]
fn fuzzy_pou_local_variables(mut with_db: RootDatabase) {
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             engine := ULINT#5;
       |             ^^^|^^
       |                `---- no item 'engine' in scope
       |
       | Note: 'fb1' has items with similar name:
       |       - engine2
       |       - no_engine
    ---'
    ");
}

#[rstest]
fn fuzzy_func_call_input_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR_INPUT
		param1: INT;
		param2: REAL;
	END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

	fn(param := 0);
END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     fn(param := 0);
        |        ^^|^^
        |          `---- unknown input 'param'
        |
        | Note: 'fn' has items with similar name:
        |       - param1
        |       - param2
    ----'
    ");
}

#[rstest]
fn fuzzy_func_call_output_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR_OUTPUT
		param1: INT;
		param2: REAL;
	END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR_OUTPUT
        param_out: INT;
    END_VAR

	fn(param => param_out);
END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     fn(param => param_out);
        |        ^^|^^
        |          `---- unknown output 'param'
        |
        | Note: 'fn' has items with similar name:
        |       - param1
        |       - param2
    ----'
    ");
}
