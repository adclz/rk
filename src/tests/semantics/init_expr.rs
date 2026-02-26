use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_ty::head::init_inference::infer_initialization;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::find_pou_with_name;
use crate::tests::utils::test_diagnostics;
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:12:49 ]
        |
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^^^^^|^^^^^  
        |                                                       `------- 'Engine' has no field named 'fuel'
    ----'
    ");
}

#[rstest]
fn invalid_struct_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (power := 10, fuel := 10.0);
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:11:48 ]
        |
     11 |                 Base : Engine := (power := 10, fuel := 10.0);
        |                                                ^^^^^^|^^^^^  
        |                                                      `------- 'Engine' has no field named 'fuel'
    ----'
    ");
}

#[rstest]
fn invalid_array_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(10.5)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [3(10.5)];
       |                                     ^^|^  
       |                                       `--- cannot infer '<float>' to 'INT': invalid INT literal
    ---'
    ");
}

#[rstest]
fn invalid_value_in_array_of_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: INT;
                Torque: INT;
            END_STRUCT;
            EngineArray: ARRAY[0..3] OF Engine;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : EngineArray := [(Power := 10, Torque := 10.0)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:12:64 ]
        |
     12 |                 Base : EngineArray := [(Power := 10, Torque := 10.0)];
        |                                                                ^^|^  
        |                                                                  `--- cannot infer '<float>' to 'INT': invalid INT literal
    ----'
    ");
}

#[rstest]
fn invalid_value_in_struct_with_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: ARRAY[0..2] OF INT;
                Torque: INT;
            END_STRUCT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (Power := [10, 5.3], Torque := 10);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:11:49 ]
        |
     11 |                 Base : Engine := (Power := [10, 5.3], Torque := 10);
        |                                                 ^|^  
        |                                                  `--- cannot infer '<float>' to 'INT': invalid INT literal
    ----'
    ");
}

#[rstest]
fn unexpected_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2(param1 := 0)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [2(param1 := 0)];
       |                                     ^^^^^|^^^^^  
       |                                          `------- 'Engine' has no field named 'param1'
    ---'
    ");
}

#[rstest]
fn unexpected_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0213] Error: invalid operation
       ,-[ file:///test0.st:8:31 ]
       |
     8 |                 Base : Engine := [2];
       |                               ^^^|^^  
       |                                  `---- cannot index into type 'Engine'
    ---'
    ");
}

/// Utility to collect all path expressions in a given source file.
/// The output is a list of lines with the format:
/// `<offset> <type>`
fn collect_init_expressions(db: &dyn WorkspaceDataBase, file: File, pou_name: &str) -> String {
    let pou = find_pou_with_name(db, file, pou_name).unwrap();

    let mut result = vec![];
    let infer_result = infer_initialization(db, pou.get_scope_id(db));

    for (init_expr, typ) in &infer_result.init_expr_result.type_of_init_expr {
        result.push(format!("{} {}", init_expr.get_id(db).id(), typ.kind()));
    }

    result.join("\n")
}

#[rstest]
fn walk_array_path_expression(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION fn
            VAR
                Base : Engine := [5];
            END_VAR

        END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(collect_init_expressions(&with_db, *with_db.get_files().iter().last().unwrap(), &"fn"), @"34 DATATYPE");
}

#[rstest]
fn struct_fields(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION fn
            VAR
                // fuel is not a member of engine
                Base : Engine := (power := 100, oil := 10.0);
            END_VAR

        END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(collect_init_expressions(&with_db, *with_db.get_files().iter().last().unwrap(), &"fn"), @r"
    28 DATATYPE
    30 STRUCT_ELEMENT
    40 STRUCT_ELEMENT
    ");
}
