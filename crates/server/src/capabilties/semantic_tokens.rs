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

pub fn semantic_tokens_full(
    db: &impl BaseDatabase,
    params: SemanticTokensParams,
) -> anyhow::Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut builder = SemanticTokensBuilder::new("".into());

    Ok(None)
}
