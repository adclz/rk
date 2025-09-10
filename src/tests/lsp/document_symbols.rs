use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::to_proto::ToProto;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::make_db_with_source;
use crate::tests::utils::with_db;

#[rstest]
pub fn pous_document_symbols(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
END_NAMESPACE

FUNCTION fn1
END_FUNCTION

FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK

CLASS class1
END_CLASS

INTERFACE in1
END_INTERFACE"#;

    let file = make_db_with_source(&mut with_db, source);
    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(&with_db, file);
    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));

    let result = builder.finalize();

    assert_debug_snapshot!(&result, @r#"
    [
        DocumentSymbol {
            name: "fn1",
            detail: Some(
                "function",
            ),
            kind: Function,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 4,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 12,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 4,
                    character: 9,
                },
                end: Position {
                    line: 4,
                    character: 12,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "fb1",
            detail: Some(
                "function block",
            ),
            kind: Function,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 7,
                    character: 0,
                },
                end: Position {
                    line: 8,
                    character: 18,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 7,
                    character: 15,
                },
                end: Position {
                    line: 7,
                    character: 18,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "class1",
            detail: Some(
                "class",
            ),
            kind: Class,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 9,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 10,
                    character: 6,
                },
                end: Position {
                    line: 10,
                    character: 12,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "in1",
            detail: Some(
                "interface",
            ),
            kind: Interface,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 13,
                    character: 0,
                },
                end: Position {
                    line: 14,
                    character: 13,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 13,
                    character: 10,
                },
                end: Position {
                    line: 13,
                    character: 13,
                },
            },
            children: Some(
                [],
            ),
        },
    ]
    "#);
}

#[rstest]
pub fn nested_variables_document_symbols(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
        VAR_INPUT
            a : INT;
        END_VAR

        VAR_OUTPUT
            b : INT;
        END_VAR

        VAR_IN_OUT
            c: STRING[0];
        END_VAR
END_FUNCTION_BLOCK"#;

    let file = make_db_with_source(&mut with_db, source);
    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(&with_db, file);
    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));

    let result = builder.finalize();

    assert_debug_snapshot!(&result, @r#"
    [
        DocumentSymbol {
            name: "fb1",
            detail: Some(
                "function block",
            ),
            kind: Function,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 13,
                    character: 18,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 1,
                    character: 15,
                },
                end: Position {
                    line: 1,
                    character: 18,
                },
            },
            children: Some(
                [
                    DocumentSymbol {
                        name: "a",
                        detail: Some(
                            "variable",
                        ),
                        kind: Variable,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 3,
                                character: 12,
                            },
                            end: Position {
                                line: 3,
                                character: 19,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 3,
                                character: 12,
                            },
                            end: Position {
                                line: 3,
                                character: 13,
                            },
                        },
                        children: None,
                    },
                    DocumentSymbol {
                        name: "b",
                        detail: Some(
                            "variable",
                        ),
                        kind: Variable,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 7,
                                character: 12,
                            },
                            end: Position {
                                line: 7,
                                character: 19,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 7,
                                character: 12,
                            },
                            end: Position {
                                line: 7,
                                character: 13,
                            },
                        },
                        children: None,
                    },
                    DocumentSymbol {
                        name: "c",
                        detail: Some(
                            "variable",
                        ),
                        kind: Variable,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 11,
                                character: 12,
                            },
                            end: Position {
                                line: 11,
                                character: 24,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 11,
                                character: 12,
                            },
                            end: Position {
                                line: 11,
                                character: 13,
                            },
                        },
                        children: None,
                    },
                ],
            ),
        },
    ]
    "#);
}

#[rstest]
pub fn class_methods_document_symbols(mut with_db: RootDatabase) {
    let source = r#"
CLASS CCounter
	METHOD Count (* Only body *)
		IF (m_bCountUp AND m_iCurrentValue<m_iUpperLimit) THEN
			m_iCurrentValue:= m_iCurrentValue+1;
		END_IF;

		IF (NOT m_bCountUp AND m_iCurrentValue>m_iLowerLimit) THEN
			m_iCurrentValue:= m_iCurrentValue-1;
		END_IF;
	END_METHOD

	METHOD SetDirection
	
        m_bCountUp:=bCountUp;

	END_METHOD
END_CLASS"#;

    let file = make_db_with_source(&mut with_db, source);
    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(&with_db, file);
    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));

    let result = builder.finalize();

    assert_debug_snapshot!(&result, @r#"
    [
        DocumentSymbol {
            name: "CCounter",
            detail: Some(
                "class",
            ),
            kind: Class,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 17,
                    character: 9,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 1,
                    character: 6,
                },
                end: Position {
                    line: 1,
                    character: 14,
                },
            },
            children: Some(
                [
                    DocumentSymbol {
                        name: "Count",
                        detail: Some(
                            "method",
                        ),
                        kind: Method,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 2,
                                character: 1,
                            },
                            end: Position {
                                line: 10,
                                character: 11,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 2,
                                character: 8,
                            },
                            end: Position {
                                line: 2,
                                character: 13,
                            },
                        },
                        children: Some(
                            [],
                        ),
                    },
                    DocumentSymbol {
                        name: "SetDirection",
                        detail: Some(
                            "method",
                        ),
                        kind: Method,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 12,
                                character: 1,
                            },
                            end: Position {
                                line: 16,
                                character: 11,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 12,
                                character: 8,
                            },
                            end: Position {
                                line: 12,
                                character: 20,
                            },
                        },
                        children: Some(
                            [],
                        ),
                    },
                ],
            ),
        },
    ]
    "#);
}

#[rstest]
pub fn interface_methods_document_symbols(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ROOM
    METHOD DAYTIME END_METHOD // Called in day-time
    METHOD NIGHTTIME END_METHOD // in night-time
END_INTERFACE"#;

    let file = make_db_with_source(&mut with_db, source);
    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(&with_db, file);
    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));

    let result = builder.finalize();

    assert_debug_snapshot!(&result, @r#"
    [
        DocumentSymbol {
            name: "ROOM",
            detail: Some(
                "interface",
            ),
            kind: Interface,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 4,
                    character: 13,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 1,
                    character: 10,
                },
                end: Position {
                    line: 1,
                    character: 14,
                },
            },
            children: Some(
                [
                    DocumentSymbol {
                        name: "DAYTIME",
                        detail: Some(
                            "method [proto]",
                        ),
                        kind: Method,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 2,
                                character: 4,
                            },
                            end: Position {
                                line: 2,
                                character: 29,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 2,
                                character: 11,
                            },
                            end: Position {
                                line: 2,
                                character: 18,
                            },
                        },
                        children: Some(
                            [],
                        ),
                    },
                    DocumentSymbol {
                        name: "NIGHTTIME",
                        detail: Some(
                            "method [proto]",
                        ),
                        kind: Method,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 3,
                                character: 4,
                            },
                            end: Position {
                                line: 3,
                                character: 31,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: 3,
                                character: 11,
                            },
                            end: Position {
                                line: 3,
                                character: 20,
                            },
                        },
                        children: Some(
                            [],
                        ),
                    },
                ],
            ),
        },
    ]
    "#);
}

#[rstest]
pub fn data_types_document_symbols(mut with_db: RootDatabase) {
    let source = r#"
TYPE 
    BOOL : BOOL;
    NUMBER : INT;
    FLOAT : REAL;
    STRING20 : STRING[20];
    ARRAY_5 : ARRAY[1..5] OF INT;
END_TYPE"#;

    let file = make_db_with_source(&mut with_db, source);
    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(&with_db, file);
    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));

    let result = builder.finalize();

    assert_debug_snapshot!(&result, @r#"
    [
        DocumentSymbol {
            name: "BOOL",
            detail: Some(
                "data type",
            ),
            kind: Boolean,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 2,
                    character: 4,
                },
                end: Position {
                    line: 2,
                    character: 15,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 2,
                    character: 4,
                },
                end: Position {
                    line: 2,
                    character: 8,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "NUMBER",
            detail: Some(
                "data type",
            ),
            kind: Number,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 3,
                    character: 4,
                },
                end: Position {
                    line: 3,
                    character: 16,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 3,
                    character: 4,
                },
                end: Position {
                    line: 3,
                    character: 10,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "FLOAT",
            detail: Some(
                "data type",
            ),
            kind: Number,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 4,
                    character: 4,
                },
                end: Position {
                    line: 4,
                    character: 16,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 4,
                    character: 4,
                },
                end: Position {
                    line: 4,
                    character: 9,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "STRING20",
            detail: Some(
                "data type",
            ),
            kind: String,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 5,
                    character: 4,
                },
                end: Position {
                    line: 5,
                    character: 25,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 5,
                    character: 4,
                },
                end: Position {
                    line: 5,
                    character: 12,
                },
            },
            children: Some(
                [],
            ),
        },
        DocumentSymbol {
            name: "ARRAY_5",
            detail: Some(
                "data type",
            ),
            kind: Array,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 6,
                    character: 4,
                },
                end: Position {
                    line: 6,
                    character: 32,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 6,
                    character: 4,
                },
                end: Position {
                    line: 6,
                    character: 11,
                },
            },
            children: Some(
                [],
            ),
        },
    ]
    "#);
}
