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
    [E0211] Error: no such field
        ,-[ file:///test0.st:15:49 ]
        |
     15 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^^^^^|^^^^^
        |                                                       `------- 'Engine' has no field named 'fuel'
        |
        | Note: 'Engine' has fields with similar name:
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             engine := ULINT#5;
       |             ^^^|^^
       |                `---- no item "engine" found in scope
       |
       | Note: 'fb1' has items with similar name:
       |       - engine2
       |       - no_engine
    ---'
    "#);
}

#[rstest]
fn fuzzy_pou_items_not_in_scope(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System
	FUNCTION fn

	END_FUNCTION

	FUNCTION fn2

	END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK fb1

	fn();

END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     fn();
        |     ^|
        |      `-- no item "fn" found in scope
        |
        | Note: items named 'fn' are available, but need to be imported:
        |       - USING System
        |       - USING System
    ----'
    "#);
}

// same test as above but with multiple items with similar name to check that the error message is not duplicated
// so we also expect a duplicate error
#[rstest]
fn deduplicate_fuzzy_pou_items_not_in_scope(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System
	FUNCTION fn

	END_FUNCTION

	FUNCTION fn

	END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK fb1

	fn();

END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0101] Error: duplicate definitions
       ,-[ file:///test0.st:3:11 ]
       |
     3 |     FUNCTION fn
       |              ^|
       |               `-- duplicate POU 'fn'
       |
     7 |     FUNCTION fn
       |              ^|
       |               `-- POU 'fn' is already defined here
    ---'
    [E0204] Error: no item found in scope
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     fn();
        |     ^|
        |      `-- no item "fn" found in scope
        |
        | Note: an item named 'fn' is available, but needs to be imported:
        |       - USING System
    ----'
    "#);
}

#[rstest]
fn fuzzy_pou_local_functions(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn

END_FUNCTION

FUNCTION fn2

    f();

END_FUNCTION
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     f();
       |     |
       |     `-- no item "f" found in scope
       |
       | Note: items with similar name available in scope:
       |       - fn
       |       - fn2
    ---'
    "#);
}

#[rstest]
fn fuzzy_namespace_target_not_in_scope(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System
	TYPE Engine: INT END_TYPE
END_NAMESPACE

FUNCTION_BLOCK fb1
    VAR
        engine: Engine;
    END_VAR

END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:8:17 ]
       |
     8 |         engine: Engine;
       |                 ^^^|^^
       |                    `---- no item found for path 'Engine'
       |
       | Note: an item named 'Engine' is available, but needs to be imported:
       |       - USING System
    ---'
    ");
}

#[rstest]
fn recovery_using_namespace_as_type(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System

END_NAMESPACE

FUNCTION_BLOCK fb1
	VAR
		engine: System;
	END_VAR

END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:8:11 ]
       |
     8 |        engine: System;
       |                ^^^|^^
       |                   `---- no item found for path 'System'
       |
       | Note: namespace named 'System' exists but it cannot be used as an item, you can either:
       |       - Import the namespace via an USING directive: 'USING System'
       |       - Import an item from this namespace: 'System.<POU>'
    ---'
    ");
}

#[rstest]
fn fuzzy_struct_path_expr(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                fuel1 : REAL;
                fuel2 : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION_BLOCK fb1
            VAR
                e : Engine;
            END_VAR

            e.fule1 := 1.0;

        END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:15:15 ]
        |
      3 | ,->             STRUCT
        : :
      7 | |->             END_STRUCT
        | |
        | `---------------------------- type is defined by 'Engine' here
        |
     15 |                 e.fule1 := 1.0;
        |                   ^^|^^
        |                     `---- 'Engine' has no field named 'fule1'
    ----'
    ");
}

#[rstest]
fn fuzzy_fb_fields(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
            VAR
                speed : INT;
                torque : REAL;
                running : BOOL;
            END_VAR

        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Controller
            VAR
                m : Motor;
            END_VAR

            m.speeed := 100;

        END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:16:15 ]
        |
      2 |         FUNCTION_BLOCK Motor
        |                        ^^|^^
        |                          `---- FUNCTION_BLOCK 'Motor' is defined here
        |
     16 |             m.speeed := 100;
        |               ^^^|^^
        |                  `---- 'Motor' has no field named 'speeed'
    ----'
    ");
}

#[rstest]
fn fuzzy_class_fields(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Pump
            VAR
                pressure : REAL;
                flowRate : REAL;
            END_VAR

        END_CLASS

        FUNCTION_BLOCK Controller
            VAR
                p : Pump;
            END_VAR

            p.presure := 1.0;

        END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:15:15 ]
        |
      2 |         CLASS Pump
        |               ^^|^
        |                 `--- CLASS 'Pump' is defined here
        |
     15 |             p.presure := 1.0;
        |               ^^^|^^^
        |                  `----- 'Pump' has no field named 'presure'
        |
        | Note: 'Pump' has field with similar name:
        |       - pressure
    ----'
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
    [E0208] Error: function call parameter mismatch
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     fn(param := 0);
        |        ^^|^^
        |          `---- unknown input parameter 'param'
        |
        | Note: 'fn' has parameters with similar name:
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
    [E0209] Error: function call parameter mismatch
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     fn(param => param_out);
        |        ^^|^^
        |          `---- unknown output parameter 'param'
        |
        | Note: 'fn' has parameters with similar name:
        |       - param1
        |       - param2
    ----'
    ");
}
