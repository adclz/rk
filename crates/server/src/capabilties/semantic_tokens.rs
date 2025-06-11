use std::sync::LazyLock;

use ast::generated::{ClassDecl, DataTypeDecl, FbDecl, FuncDecl, InterfaceDecl, NamespaceDecl};
use auto_lsp::{
    anyhow,
    core::{ast::AstNode, dispatch, semantic_tokens_builder::SemanticTokensBuilder},
    default::db::{tracked::get_ast, BaseDatabase, File},
    define_semantic_token_modifiers, define_semantic_token_types,
    lsp_types::{self, SemanticTokenModifier, SemanticTokensParams, SemanticTokensResult},
    tree_sitter::{self, StreamingIterator},
};

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION,
        CLASS,
        INTERFACE,
        TYPE,
        VARIABLE,
        KEYWORD,
        MODIFIER
    }

    custom {

    }
];

define_semantic_token_modifiers![
    standard {
        DECLARATION,
        MODIFICATION
    }

    custom {
        (INTERNAL, "internal"),
        (CONTROL, "control"),
        (OOP, "oop"),
    }
];

static HIGHLIGHT_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        r#"
        [
         "NAMESPACE"
         "END_NAMESPACE"
         "FUNCTION"
         "END_FUNCTION"
         "FUNCTION_BLOCK"
         "END_FUNCTION_BLOCK"
         "TYPE"
         "END_TYPE"
         "CLASS"
         "END_CLASS"
         "INTERFACE"
         "END_INTERFACE"
         "VAR_INPUT"
         "VAR_OUTPUT"
         "VAR_IN_OUT"
         "VAR_TEMP"
         "VAR_EXTERNAL"
         "VAR_GLOBAL"
         "VAR"
         "END_VAR"
        ] @keyword

        ["PUBLIC" "PROTECTED" "PRIVATE" "INTERNAL"] @keyword.modifiers
        ["USING" "EXTENDS" "IMPLEMENTS"] @keyword.oop
        ["+" "-" ":=" "=" ":"] @keyword.control

        (using_directive (_) @declaration.namespace)
        (namespace_decl name: (_) @declaration.namespace)
        (func_decl name: (_) @declaration.function)
        (fb_decl name: (_) @declaration.function_block)
        (class_decl name: (_) @declaration.class)
        "#,
    )
    .unwrap()
});

#[derive(Default)]
pub(crate) struct ModifierSet(pub(crate) u32);

impl std::ops::BitOrAssign<SemanticTokenModifier> for ModifierSet {
    fn bitor_assign(&mut self, rhs: SemanticTokenModifier) {
        let idx = SUPPORTED_MODIFIERS
            .iter()
            .position(|it| it == &rhs)
            .unwrap();
        self.0 |= 1 << idx;
    }
}

pub fn semantic_tokens_full(
    db: &impl BaseDatabase,
    params: SemanticTokensParams,
) -> anyhow::Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut builder = SemanticTokensBuilder::new("".into());

    let doc = file.document(db);
    let root_node = doc.tree.root_node();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures =
        query_cursor.captures(&HIGHLIGHT_QUERY, root_node, doc.texter.text.as_bytes());

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        let range = capture.node.range();

        let parse_captures = HIGHLIGHT_QUERY.capture_names()[capture.index as usize]
            .split(".")
            .collect::<Vec<_>>();

        let mut modifiers = ModifierSet::default();
        let token_idx = match parse_captures[0] {
            "keyword" => {
                if parse_captures.len() > 1 {
                    match parse_captures[1] {
                        "modifiers" => {
                            SUPPORTED_TYPES.iter().position(|x| *x == MODIFIER).unwrap() as u32
                        }
                        "oop" => {
                            modifiers |= OOP;
                            SUPPORTED_TYPES.iter().position(|x| *x == KEYWORD).unwrap() as u32
                        }
                        "control" => {
                            modifiers |= CONTROL;
                            SUPPORTED_TYPES.iter().position(|x| *x == KEYWORD).unwrap() as u32
                        }
                        _ => SUPPORTED_TYPES.iter().position(|x| *x == KEYWORD).unwrap() as u32,
                    }
                } else {
                    SUPPORTED_TYPES.iter().position(|x| *x == KEYWORD).unwrap() as u32
                }
            }
            "declaration" => {
                modifiers |= DECLARATION;
                match parse_captures[1] {
                    "namespace" => SUPPORTED_TYPES
                        .iter()
                        .position(|x| *x == NAMESPACE)
                        .unwrap() as u32,
                    "function" => {
                        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32
                    }
                    "function_block" => {
                        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32
                    }
                    "class" => SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
                    "interface" => SUPPORTED_TYPES
                        .iter()
                        .position(|x| *x == INTERFACE)
                        .unwrap() as u32,
                    "type" => SUPPORTED_TYPES.iter().position(|x| *x == TYPE).unwrap() as u32,
                    _ => continue,
                }
            }
            _ => continue,
        };

        builder.push(
            lsp_types::Range {
                start: lsp_types::Position::new(
                    range.start_point.row as u32,
                    range.start_point.column as u32,
                ),
                end: lsp_types::Position::new(
                    range.end_point.row as u32,
                    range.end_point.column as u32,
                ),
            },
            token_idx,
            modifiers.0,
        );
    }

    Ok(Some(SemanticTokensResult::Tokens(builder.build())))
}
