use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::TypeInfo;
use hir::hir_def::semantic_index::HirNode;
use hir::hir_def::semantic_index::SemanticIndex;
use hir::hir_def::semantic_index::semantic_index;
use hir::walk::WalkHir;
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
        if let HirNode::ResolvedPath(path) = n {
            result.push(path);
        }
        ControlFlow::Continue(())
    });

    result
        .iter()
        .map(|r| {
            format!(
                "{} {}",
                r.get_span(db).start_byte,
                r.try_to_ty(db).unwrap().type_name(db)
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
    Error: 
       ,-[ file:///test0.st:7:13 ]
       |
     4 |        test: ARRAY[0..2] OF INT;
       |                             ^|^  
       |                              `--- expected type 'INT' here
       | 
     7 |     test[0] := 0.2;
       |                ^|^  
       |                 `--- invalid assignment: invalid INT literal
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
    Error: 
        ,-[ file:///test0.st:14:7 ]
        |
      2 | ,-> TYPE Engine:
        : :   
      6 | |->     END_STRUCT
        | |                    
        | `--------------------  'STRUCT' declared here
        | 
     14 |         test.powerr := 0.2;
        |              ^^^|^^  
        |                 `---- field 'powerr' not found in 'STRUCT'
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
    Error: 
        ,-[ file:///test0.st:14:16 ]
        |
      4 |         power : INT;
        |                 ^|^  
        |                  `--- expected type 'INT' here
        | 
     14 |     test.power := 0.2;
        |                   ^|^  
        |                    `--- invalid assignment: invalid INT literal
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
    Error: 
        ,-[ file:///test0.st:14:2 ]
        |
      2 | ,-> TYPE Engine:
        : :   
      6 | |->     END_STRUCT
        | |                    
        | `--------------------  'STRUCT' declared here
        | 
     14 |         test[0] := 0.2;
        |         ^^|^  
        |           `--- type 'STRUCT' cannot be indexed
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
     4 |        test: ARRAY[0..1] OF INT;
       |            ^^^^^^^^^^|^^^^^^^^^  
       |                      `-----------  'ARRAY [0..1] OF INT' declared here
       | 
     7 |     test.not_a_field := 0.2;
       |          ^^^^^|^^^^^  
       |               `------- type 'ARRAY [0..1] OF INT' does not have fields
    ---'
    ");
}
