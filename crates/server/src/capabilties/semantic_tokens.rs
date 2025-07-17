use std::sync::LazyLock;

use auto_lsp::{
    anyhow,
    core::semantic_tokens_builder::SemanticTokensBuilder,
    default::db::BaseDatabase,
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
        MODIFIER,
        OPERATOR,
        TYPE_PARAMETER,
        NUMBER,
        STRING,
        METHOD
    }

    custom {

    }
];

define_semantic_token_modifiers![
    standard {
        DECLARATION,
        MODIFICATION,

    }

    custom {
        (INTERNAL, "internal"),
        (CONTROL, "control"),
        (OOP, "oop"),
    }
];

static QUERY: &str = r#"
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
         "METHOD"
         "END_METHOD"
        ] @keyword

        (variable_list (identifier) @variable) 
        (type_decl name: (identifier) @declaration.type)

        ["PUBLIC" "PROTECTED" "PRIVATE" "INTERNAL"
            (assignment) (assignment_attempt)
        ] @keyword.modifiers
        
        [
         "VAR_INPUT"
         "VAR_OUTPUT"
         "VAR_IN_OUT"
         "VAR_TEMP"
         "VAR_EXTERNAL"
         "VAR_GLOBAL"
         "VAR"
         "END_VAR" 
         "USING" 
         "EXTENDS" 
         "IMPLEMENTS"
        ] @keyword.oop
        ["+" "-" ":=" "=" ":"] @keyword.control
        [ 
	        (bool_name)
            (byte_name) (word_name) (dword_name) (lword_name)
            (sint_name) (int_name) (dint_name) (lint_name)
            (usint_name) (uint_name) (udint_name) (ulint_name)
            (real_name) (lreal_name)   
        ] @type_parameter

        ((_) @int (#match? @int "^[0-9_]+")) @int
        (char_str) @string

        (using_directive (_) @declaration.namespace)
        (namespace_decl name: (_) @declaration.namespace)
        (func_decl name: (_) @declaration.function)
        (fb_decl name: (_) @declaration.function_block)
        (class_decl name: (_) @declaration.class)
        (interface_decl name: (_) @declaration.interface)
        (method_prototype name: (_) @declaration.method)
"#;

static HIGHLIGHT_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), QUERY).unwrap()
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
            "type_parameter" => SUPPORTED_TYPES
                .iter()
                .position(|x| *x == TYPE_PARAMETER)
                .unwrap() as u32,
            "variable" => SUPPORTED_TYPES.iter().position(|x| *x == VARIABLE).unwrap() as u32,
            "operator" => SUPPORTED_TYPES.iter().position(|x| *x == OPERATOR).unwrap() as u32,
            "int" => SUPPORTED_TYPES.iter().position(|x| *x == NUMBER).unwrap() as u32,
            "string" => SUPPORTED_TYPES.iter().position(|x| *x == STRING).unwrap() as u32,
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
                    "method" => SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
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

#[cfg(test)]
mod tests {
    use auto_lsp::tree_sitter;

    use super::*;

    #[test]
    fn load_formatting_query() {
        tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), QUERY)
            .expect("Failed to create formatting query");
    }
}
