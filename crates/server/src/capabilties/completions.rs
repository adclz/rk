use ast::generated::DataTypeDecl;
use auto_lsp::{
    anyhow,
    default::db::tracked::get_ast,
    lsp_types::{CompletionParams, CompletionResponse},
};
use db::WorkspaceDataBase;
use ide_proto::{
    comment_index::comment_index,
    handlers::{CompletionHandler, CompletionRequest},
    walk::completion_descendant_at,
};

pub fn completions(
    db: &impl WorkspaceDataBase,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let doc = file.document(db);

    let position = params.text_document_position.position;
    let trigger_character = params
        .context
        .as_ref()
        .and_then(|ctx| ctx.trigger_character.clone());

    let offset = match doc.offset_at(position) {
        Some(offset) => offset,
        None => return Ok(None),
    };

    // Suppress completions inside comments
    // todo: can binary_search be used here instead of iterating over all comments?
    let in_comment = comment_index(db, file)
        .map
        .values()
        .any(|comment| comment.range.start_byte <= offset && offset <= comment.range.end_byte);

    if in_comment {
        return Ok(Some(CompletionResponse::Array(vec![])));
    }

    let (target, node_key, is_last_before) = match completion_descendant_at(db, file, offset) {
        Some(result) => result,
        None => {
            // Check if cursor is inside a TYPE declaration at the AST level.
            // When the TYPE body is incomplete (no spec yet), no HIR node covers
            // the cursor, but we should still offer type-level completions
            // instead of POU-level snippets.

            // TODO: this is extremly hacky, descendant_at should be fixed in auto_lsp to avoid instead of any
            let ast = get_ast(db, file);
            let in_type_decl = ast.iter().any(|node| {
                let range = node.get_range();
                range.start_byte <= offset
                    && offset <= range.end_byte
                    && node.lower().downcast_ref::<DataTypeDecl>().is_some()
            });

            if in_type_decl {
                return Ok(Some(CompletionResponse::Array(vec![])));
            }

            // no target node, show general completions (namespaces, pou snippets, etc)
            return Ok(Some(CompletionResponse::Array(vec![
                ide_proto::handlers::completions_utils::static_snippets::namespace(),
                ide_proto::handlers::completions_utils::static_snippets::using(),
                ide_proto::handlers::completions_utils::static_snippets::function(),
                ide_proto::handlers::completions_utils::static_snippets::function_block(),
                ide_proto::handlers::completions_utils::static_snippets::program(),
                ide_proto::handlers::completions_utils::static_snippets::class(),
                ide_proto::handlers::completions_utils::static_snippets::interface(),
                ide_proto::handlers::completions_utils::static_snippets::type_(),
                ide_proto::handlers::completions_utils::static_snippets::configuration(),
            ])));
        }
    };
    let req = CompletionRequest {
        offset,
        trigger_character,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    Ok(Some(CompletionResponse::Array(
        target.completion(db, &req).unwrap_or_default(),
    )))
}
