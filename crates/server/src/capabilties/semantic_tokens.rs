use auto_lsp::{
    anyhow,
    core::semantic_tokens_builder::SemanticTokensBuilder,
    default::db::BaseDatabase,
    define_semantic_token_modifiers, define_semantic_token_types,
    lsp_types::{SemanticTokensParams, SemanticTokensResult},
};
use hir::{hir_def::semantic_index::semantic_index, walk::WalkHir};
use ide_proto::AsProtocol;


pub fn semantic_tokens_full(
    db: &impl BaseDatabase,
    params: SemanticTokensParams,
) -> anyhow::Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let mut builder = SemanticTokensBuilder::new("".into());
    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node| {
        node.as_proto().semantic_tokens(db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();
    Ok(Some(SemanticTokensResult::Tokens(result)))
}
