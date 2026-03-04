use auto_lsp::{
    anyhow,
    core::semantic_tokens_builder::SemanticTokensBuilder,
    lsp_types::{
        SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
        SemanticTokensResult,
    },
};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::semantic_index::semantic_index};
use ide_proto::{handlers::SemanticTokensHandler, walk::WalkHir};

pub fn semantic_tokens_full(
    db: &impl WorkspaceDataBase,
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
        node.semantic_tokens(db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();
    Ok(Some(SemanticTokensResult::Tokens(result)))
}

pub fn semantic_tokens_range(
    db: &impl WorkspaceDataBase,
    params: SemanticTokensRangeParams,
) -> anyhow::Result<Option<SemanticTokensRangeResult>> {
    let uri = &params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let document = file.document(db);

    let start_offset = document.offset_at(params.range.start).ok_or_else(|| {
        anyhow::format_err!("Invalid range start, {:?}", params.range.start)
    })?;
    let end_offset = document.offset_at(params.range.end).ok_or_else(|| {
        anyhow::format_err!("Invalid range end, {:?}", params.range.end)
    })?;

    let mut builder = SemanticTokensBuilder::new("".into());
    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node| {
        let span = node.get_span(db);

        // Skip nodes entirely before the range
        if span.end_byte < start_offset {
            return std::ops::ControlFlow::Continue(());
        }

        // Stop once we've passed the range
        if span.start_byte > end_offset {
            return std::ops::ControlFlow::Break(());
        }

        node.semantic_tokens(db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();
    Ok(Some(SemanticTokensRangeResult::Tokens(result)))
}
