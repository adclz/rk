use auto_lsp::{
    anyhow,
    core::document_symbols_builder::DocumentSymbolsBuilder,
    lsp_types::{DocumentSymbolParams, DocumentSymbolResponse},
};
use db::WorkspaceDataBase;
use hir::hir_def::scope::ScopeKind;
use hir::hir_def::semantic_index::{get_scope, semantic_index};
use ide_proto::handlers::DocumentSymbolsHandler;

pub fn document_symbols(
    db: &impl WorkspaceDataBase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(db, file);

    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(db, &mut builder));
    sema.programs
        .iter()
        .for_each(|program| program.document_symbols(db, &mut builder));
    sema.namespaces
        .iter()
        .filter(|ns| {
            let scope = get_scope(db, ns.scope_id(db));
            matches!(
                scope.parent.map(|p| get_scope(db, p).kind),
                Some(ScopeKind::Global) | None
            )
        })
        .for_each(|ns| ns.document_symbols(db, &mut builder));
    sema.configs
        .iter()
        .for_each(|config| config.document_symbols(db, &mut builder));

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
