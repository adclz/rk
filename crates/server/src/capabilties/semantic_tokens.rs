use auto_lsp::{
    anyhow,
    core::semantic_tokens_builder::SemanticTokensBuilder,
    default::db::BaseDatabase,
    define_semantic_token_modifiers, define_semantic_token_types,
    lsp_types::{SemanticTokensParams, SemanticTokensResult},
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

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let builder = SemanticTokensBuilder::new("".into());

    Ok(None)
}
