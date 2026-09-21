use std::ops::ControlFlow;

use auto_lsp::lsp_types::{InlayHint, InlayHintLabel};
use db::RootDatabase;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::InlayHintHandler;
use ide_proto::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_source;
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let result: Vec<InlayHint> = sema
        .global_pous
        .iter()
        .filter_map(|pou| pou.inlay_hint(&with_db))
        .collect();

    assert_snapshot!(render(&result), @r"
    3:12  FUNCTION fn1  -> 2:9
    6:18  FUNCTION_BLOCK fb1  -> 5:15
    9:9  CLASS class1  -> 8:6
    12:13  INTERFACE in1  -> 11:10
    ");
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

    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let result: Vec<InlayHint> = sema
        .namespaces
        .iter()
        .filter_map(|ns| ns.inlay_hint(&with_db))
        .collect();

    assert_snapshot!(render(&result), @r"
    5:17  NAMESPACE ns1.nested  -> 3:14
    7:13  NAMESPACE ns1  -> 2:10
    11:13  NAMESPACE ns2  -> 9:10
    ");
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

    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db)
        {
            result.push(inlay_hint);
        }
        ControlFlow::Continue(())
    });

    // Formal parameters should not generate inlay hints
    assert_snapshot!(render(&result), @"");
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

    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db)
        {
            result.push(inlay_hint);
        }
        ControlFlow::Continue(())
    });

    // Non-formal parameters should show their formal name
    assert_snapshot!(render(&result), @r"
    10:7  param1:  -> 4:8
    10:10  param2:  -> 5:8
    ");
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

    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::Param(stmt) = n
            && let Some(inlay_hint) = stmt.inlay_hint(&with_db)
        {
            result.push(inlay_hint);
        }
        ControlFlow::Continue(())
    });

    // Non-variadic shows name, variadic shows position
    assert_snapshot!(render(&result), @r"
    10:7  param1:  -> 4:8
    10:10  (1):
    10:13  (2):
    10:16  (3):
    ");
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

    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |n| {
        if let HirNode::InitExpr(curr) = n &&
            let Some(inlay_hint) = curr.inlay_hint(&with_db) {
                result.push(inlay_hint);
            
        }
        ControlFlow::Continue(())
    });

    assert_snapshot!(render(&result), @r"
    11:24  : ARRAY [0..2] OF INT
    11:39  : INT
    ");
}

/// The `: TYPE` a struct field's hint shows is a link to where that type is
/// declared. Resolving the field instead would send the reader back to the
/// line the hint already sits on, and an elementary type is declared nowhere.
#[rstest]
pub fn a_type_hint_links_to_its_declaration(mut with_db: RootDatabase) {
    let source = r#"
TYPE Inner :
STRUCT
    depth : INT;
END_STRUCT
END_TYPE

TYPE Engine :
STRUCT
    oil : REAL;
    sub : Inner;
END_STRUCT
END_TYPE

PROGRAM prog
VAR
    e : Engine := (oil := 1.0, sub := (depth := 2));
END_VAR
END_PROGRAM
"#;
    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);
    let mut result = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node: HirNode| {
        if let HirNode::InitExpr(init) = node
            && let Some(hint) = init.inlay_hint(&with_db)
        {
            result.push(hint);
        }
        ControlFlow::Continue(())
    });

    assert_snapshot!(render(&result), @r"
    17:22  : REAL
    17:34  : Inner  -> 2:5
    17:44  : INT
    ");
}

/// One line per hint: where it sits, what it reads, and where its link goes.
/// The debug form of an `InlayHint` is mostly the same file URL repeated,
/// which buried what a reader is checking.
fn render(hints: &[InlayHint]) -> String {
    hints
        .iter()
        .map(|hint| {
            let at = format!("{}:{}", hint.position.line + 1, hint.position.character);
            match &hint.label {
                InlayHintLabel::String(text) => format!("{at}  {text}"),
                InlayHintLabel::LabelParts(parts) => {
                    let text: String = parts.iter().map(|p| p.value.as_str()).collect();
                    match parts.iter().find_map(|p| p.location.as_ref()) {
                        Some(l) => format!(
                            "{at}  {text}  -> {}:{}",
                            l.range.start.line + 1,
                            l.range.start.character
                        ),
                        None => format!("{at}  {text}"),
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
