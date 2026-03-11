use auto_lsp::{
    anyhow,
    lsp_types::{WorkspaceSymbolParams, WorkspaceSymbolResponse},
};
use db::WorkspaceDataBase;

pub fn workspace_symbols(
    db: &impl WorkspaceDataBase,
    params: WorkspaceSymbolParams,
) -> anyhow::Result<Option<WorkspaceSymbolResponse>> {
    let results = ide_proto::handlers::workspace_symbols::workspace_symbols(db, &params.query);

    if results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(WorkspaceSymbolResponse::Nested(results)))
    }
}
