use ast::generated::NamespaceDecl;
use auto_lsp::{anyhow, core::{ast::AstNode, dispatch, document_symbols_builder::DocumentSymbolsBuilder, salsa::db::{BaseDatabase, File}, semantic_tokens_builder::SemanticTokensBuilder}, define_semantic_token_types};

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION
    }

    custom {

    }
];

pub fn dispatch_semantic_tokens(
    db: &impl BaseDatabase,
    file: File,
    node: &dyn AstNode,
    builder: &mut SemanticTokensBuilder,
) -> anyhow::Result<()> {
    dispatch!(
        node,
        [
            NamespaceDecl => tokens(db, file, builder)
        ]
    );
    Ok(())
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