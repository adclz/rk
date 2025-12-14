use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::semantic_index::SemanticIndex;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::body_inference::infer_body_scope;
use ide_proto::to_proto::hir_node::HirNode;
use ide_proto::to_proto::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Utility to collect all path expressions in a given source file.
/// The output is a list of lines with the format:
/// `<offset> <type>`
fn collect_path_expressions(db: &dyn BaseDatabase, sema: &SemanticIndex) -> String {
    let mut result = vec![];
    let _ = sema.walk_hir(db, &mut |n| {
        if let HirNode::PathExpr(path) = n {
            result.push(path);
        }
        ControlFlow::Continue(())
    });

    result
        .iter()
        .map(|r| {
            let infer_result = infer_body_scope(db, r.scope_id(db));

            format!(
                "{} {}",
                r.get_span(db).start_byte,
                infer_result.type_of_path_expr_with_adjustments(*r).unwrap().type_name(db)
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

// Both tests below ensure that we correctly walk into path expressions.

#[rstest]
fn walk_array_path_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF INT;
	END_VAR

	test[0] := 0.2;

END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // in case of index access, the index expression has the same offset as the parent expression
    assert_snapshot!(collect_path_expressions(&with_db, sema), @"");
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
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    assert_snapshot!(collect_path_expressions(&with_db, sema), @"");
}

#[rstest]
fn invalid_type_access_array_index(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF INT;
	END_VAR

	test[0] := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
       ,-[ file:///test0.st:7:2 ]
       |
     7 |     test[0] := 0.2;
       |     ^^|^  
       |       `--- cannot index non-array type 'ARRAY [0..2] OF INT'
    ---'
    Advice: 
       ,-[ file:///test0.st:7:2 ]
       |
     7 |     test[0] := 0.2;
       |     ^^^|^^^  
       |        `----- cannot use direct type '{unknown}' here
    ---'
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
    Advice: 
        ,-[ file:///test0.st:14:7 ]
        |
     14 |     test.powerr := 0.2;
        |          ^^^|^^  
        |             `---- 'STRUCT (2 members)' has no field named 'powerr'
    ----'
    Advice: 
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test.powerr := 0.2;
        |     ^^^^^|^^^^^  
        |          `------- cannot use direct type '{unknown}' here
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test.power := 0.2;
        |     ^^^^^|^^^^  
        |          `------ cannot use direct type 'INT' here
    ----'
    ");
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
    Advice: 
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test[0] := 0.2;
        |     ^^|^  
        |       `--- cannot index non-array type 'STRUCT'
    ----'
    Advice: 
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test[0] := 0.2;
        |     ^^^|^^^  
        |        `----- cannot use direct type '{unknown}' here
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
    Advice: 
       ,-[ file:///test0.st:7:7 ]
       |
     7 |     test.not_a_field := 0.2;
       |          ^^^^^|^^^^^  
       |               `------- 'ARRAY [0..1] OF INT' has no field named 'not_a_field'
    ---'
    Advice: 
       ,-[ file:///test0.st:7:2 ]
       |
     7 |     test.not_a_field := 0.2;
       |     ^^^^^^^^|^^^^^^^  
       |             `--------- cannot use direct type '{unknown}' here
    ---'
    ");
}
