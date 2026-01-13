use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_ty::body_inference::infer_body_scope;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::find_pou_with_name;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Utility to collect all path expressions in a given source file.
/// The output is a list of lines with the format:
/// `<offset> <type>`
fn collect_path_expressions(db: &dyn WorkspaceDataBase, file: File, pou_name: &str) -> String {
    let pou = find_pou_with_name(db, file, pou_name).unwrap();

    let mut result = vec![];
    let infer_result = infer_body_scope(db, pou.get_scope_id(db));

    for (path_expr, typ) in &infer_result.type_of_path_expr {
        result.push(format!(
            "{} {} {}",
            path_expr.get_id(db).id(),
            typ.kind(),
            match infer_result
                .adjustments_of_path_expr(*path_expr)
                .iter()
                .last()
            {
                Some(adj) => format!("{:?}", adj),
                None => "<none>".to_string(),
            }
        ));
    }

    result.join("\n")
}

// Both tests below ensure that we correctly walk into path expressions.

#[rstest]
fn walk_array_path_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF INT;
	END_VAR

	test[0] := 0;

END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);

    // in case of index access, the index expression has the same offset as the parent expression
    assert_snapshot!(collect_path_expressions(&with_db, *with_db.get_files().iter().last().unwrap(), &"fn"), @r"
    38 VARIABLE [Adjustment { kind: Index, target: Elementary(Int) }]
    38 VARIABLE [Adjustment { kind: Index, target: Elementary(Int) }]
    ");
}

#[rstest]
fn walk_struct_with_array_path(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : ARRAY[1..10] OF INT;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.power[0] := 0.2;

END_FUNCTION
        "#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(collect_path_expressions(&with_db, *with_db.get_files().iter().last().unwrap(), &"fn"), @r"
    51 STRUCT_ELEMENT [Adjustment { kind: Index, target: Elementary(Int) }]
    49 VARIABLE <none>
    51 STRUCT_ELEMENT [Adjustment { kind: Index, target: Elementary(Int) }]
    ");
}

#[rstest]
fn invalid_type_access_array_index(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF BOOL;
	END_VAR

	test[0] := 0.5;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test[0] := 0.5;
       |     ^^^|^^^    ^|^  
       |        `------------ 'BOOL' is expected due to this
       |                 |   
       |                 `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ---'
    ");
    assert_snapshot!(collect_path_expressions(&with_db, *with_db.get_files().iter().last().unwrap(), &"fn"), @r"
    36 VARIABLE [Adjustment { kind: Index, target: Elementary(Bool) }]
    36 VARIABLE [Adjustment { kind: Index, target: Elementary(Bool) }]
    ");
}

#[rstest]
fn invalid_struct_field_access(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.powerr := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:14:7 ]
        |
     14 |     test.powerr := 0.2;
        |          ^^^|^^  
        |             `---- 'STRUCT' has no field named 'powerr'
    ----'
    ");
}

#[rstest]
fn invalid_struct_field_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.power := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn index_expression_on_struct(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test[0] := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test[0] := 0.2;
        |     ^^|^  
        |       `--- cannot index non-array type 'STRUCT'
    ----'
    ");
}

#[rstest]
fn field_expression_on_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..1] OF INT;
	END_VAR

	test.not_a_field := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:7 ]
       |
     7 |     test.not_a_field := 0.2;
       |          ^^^^^|^^^^^  
       |               `------- 'ARRAY [0..1] OF INT' has no field named 'not_a_field'
    ---'
    ");
}
