use ast::generated::NamespaceDecl;
use auto_lsp::{anyhow, core::{ast::AstNode, dispatch, document_symbols_builder::DocumentSymbolsBuilder, semantic_tokens_builder::SemanticTokensBuilder}, default::db::{tracked::get_ast, BaseDatabase, File}, define_semantic_token_types, lsp_types::{SemanticTokensParams, SemanticTokensResult}};

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION
    }

    custom {

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

    get_ast(db, file).iter().try_for_each(|node| {
        dispatch!(
            node.lower(),
            [
                NamespaceDecl => tokens(db, file, &mut builder)
            ]
        );
        anyhow::Ok(())
    })?;
    Ok(Some(SemanticTokensResult::Tokens(builder.build())))
}

trait Tokens {
    fn tokens(&self, db: &impl BaseDatabase, file: File, builder: &mut SemanticTokensBuilder) -> anyhow::Result<()>;
}

impl Tokens for NamespaceDecl {
    fn tokens(&self, db: &impl BaseDatabase, file: File, builder: &mut SemanticTokensBuilder) -> anyhow::Result<()> {
        builder.push(self.name.get_lsp_range(), 
        SUPPORTED_TYPES.iter().position(|x| *x == NAMESPACE).unwrap() as u32, 
        0);
        Ok(())
    }
}