use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::InlayHint;
use db::RootDatabase;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::InlayHintHandler;
use ide_proto::walk::WalkHir;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn pous_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
END_FUNCTION

FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK

CLASS class1
END_CLASS

INTERFACE in1
END_INTERFACE"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let result: Vec<InlayHint> = sema
        .global_pous
        .iter()
        .filter_map(|pou| pou.inlay_hint(&with_db))
        .collect();

    assert_debug_snapshot!(&result, @r#"
    [
        InlayHint {
            position: Position {
                line: 2,
                character: 12,
            },
            label: String(
                "FUNCTION fn1",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
        InlayHint {
            position: Position {
                line: 5,
                character: 18,
            },
            label: String(
                "FUNCTION_BLOCK fb1",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
        InlayHint {
            position: Position {
                line: 8,
                character: 9,
            },
            label: String(
                "CLASS class1",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
        InlayHint {
            position: Position {
                line: 11,
                character: 13,
            },
            label: String(
                "INTERFACE in1",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
    ]
    "#);
}

#[rstest]
pub fn namespace_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    NAMESPACE nested

    END_NAMESPACE

END_NAMESPACE

NAMESPACE ns2

END_NAMESPACE"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let result: Vec<InlayHint> = sema
        .namespaces
        .iter()
        .filter_map(|ns| ns.inlay_hint(&with_db))
        .collect();

    assert_debug_snapshot!(&result, @r#"
    [
        InlayHint {
            position: Position {
                line: 4,
                character: 17,
            },
            label: String(
                "NAMESPACE ns1.nested",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
        InlayHint {
            position: Position {
                line: 6,
                character: 13,
            },
            label: String(
                "NAMESPACE ns1",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
        InlayHint {
            position: Position {
                line: 10,
                character: 13,
            },
            label: String(
                "NAMESPACE ns2",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                true,
            ),
            padding_right: None,
            data: None,
        },
    ]
    "#);
}

#[rstest]
pub fn func_call_input_params_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1 : BYTE;
        param2 : INT;
        param3 : REAL;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        param1 := 0,
        param2 := 0,
        param3 := 0.0
    );
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db) {
                result.push(inlay_hint);
            }
        ControlFlow::Continue(())
    });

    // Formal parameters should not generate inlay hints
    assert_debug_snapshot!(&result, @"[]");
}

#[rstest]
pub fn non_formal_params_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1 : BYTE;
        param2 : INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(0, 0);
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db) {
                result.push(inlay_hint);
            }
        ControlFlow::Continue(())
    });

    // Non-formal parameters should show their formal name
    assert_debug_snapshot!(&result, @r#"
    [
        InlayHint {
            position: Position {
                line: 9,
                character: 7,
            },
            label: String(
                "param1:",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
        InlayHint {
            position: Position {
                line: 9,
                character: 10,
            },
            label: String(
                "param2:",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
    ]
    "#);
}

#[rstest]
pub fn variadic_params_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1 : INT;
        args : INT...;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(1, 2, 3, 4);
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db) {
                result.push(inlay_hint);
            }
        ControlFlow::Continue(())
    });

    // Non-variadic shows name, variadic shows position
    assert_debug_snapshot!(&result, @r#"
    [
        InlayHint {
            position: Position {
                line: 9,
                character: 7,
            },
            label: String(
                "param1:",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
        InlayHint {
            position: Position {
                line: 9,
                character: 10,
            },
            label: String(
                "(1):",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
        InlayHint {
            position: Position {
                line: 9,
                character: 13,
            },
            label: String(
                "(2):",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
        InlayHint {
            position: Position {
                line: 9,
                character: 16,
            },
            label: String(
                "(3):",
            ),
            kind: Some(
                Parameter,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                true,
            ),
            data: None,
        },
    ]
    "#);
}

#[rstest]
pub fn init_expr_inlay_hints(mut with_db: RootDatabase) {
    let source = r#"
TYPE
	Engine: STRUCT
		Power: ARRAY[0..2] OF INT;
		Torque: INT;
	END_STRUCT;
END_TYPE

FUNCTION fn1
	VAR
		Base: Engine := (Power := [0], Torque := 10.0);
	END_VAR
END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::InitExpr(curr) = n {
            if let Some(inlay_hint) = curr.inlay_hint(&with_db) {
                result.push(inlay_hint);
            }
        }
        ControlFlow::Continue(())
    });

    assert_debug_snapshot!(&result, @r#"
    [
        InlayHint {
            position: Position {
                line: 10,
                character: 24,
            },
            label: String(
                ": ARRAY [0..2] OF INT",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                false,
            ),
            data: None,
        },
        InlayHint {
            position: Position {
                line: 10,
                character: 39,
            },
            label: String(
                ": INT",
            ),
            kind: Some(
                Type,
            ),
            text_edits: None,
            tooltip: None,
            padding_left: Some(
                false,
            ),
            padding_right: Some(
                false,
            ),
            data: None,
        },
    ]
    "#);
}
