use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use auto_lsp::lsp_types::InlayHint;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::to_proto::ToProto;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::make_db_with_source;
use crate::tests::utils::test_diagnostic;
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

    let file = make_db_with_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let result: Vec<InlayHint> = sema.global_pous
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
                "function fn1",
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
                "function block fb1",
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
                "class class1",
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
                "interface in1",
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

    let file = make_db_with_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let result: Vec<InlayHint> = sema.namespaces
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
                "namespace ns1.nested",
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
                "namespace ns1",
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
                "namespace ns2",
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
