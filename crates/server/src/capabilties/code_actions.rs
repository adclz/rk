use ast::generated::NamespaceDecl;
use auto_lsp::{anyhow, core::{dispatch, dispatch_once}, default::db::{tracked::get_ast, BaseDatabase, File}, lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams}};

pub fn code_actions(db: &impl BaseDatabase, params: CodeActionParams) -> anyhow::Result<Option<Vec<CodeActionOrCommand>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let document = file.document(db);
    let offset = document.offset_at(range.start).unwrap();

    let mut results = vec![];

    if let Some(node) = get_ast(db, file).descendant_at(offset) {
        dispatch!(node.lower(),
            [
                NamespaceDecl => code_actions(db, file, &mut results)
            ]
        );
    }
    Ok(Some(results))
}


pub trait GetCodeActions {
    fn code_actions(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<CodeActionOrCommand>) -> anyhow::Result<()>;
}


impl GetCodeActions for ast::generated::NamespaceDecl {
    fn code_actions(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<CodeActionOrCommand>) -> anyhow::Result<()> {
        if self.internal.is_some() {
            results.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Remove internal".to_string(),
                kind: Some(CodeActionKind::REFACTOR),
                diagnostics: None,
                is_preferred: None,
                edit: None,
                command: None,
                data: None,
                disabled: None,
            }));
        } else {
            results.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Add internal".to_string(),
                kind: Some(CodeActionKind::REFACTOR),
                diagnostics: None,
                is_preferred: None,
                edit: None,
                command: None,
                data: None,
                disabled: None,
            }));
        }
        Ok(())
    }
}