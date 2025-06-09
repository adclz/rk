use ast::generated::NamespaceDecl;
use auto_lsp::{anyhow, core::dispatch, default::db::{tracked::get_ast, BaseDatabase, File}, lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeLens, CodeLensParams, Command}};
use auto_lsp::core::ast::AstNode;
use db::namespaces::{namespace_path, namespace_solver};

pub fn code_lens(db: &impl BaseDatabase, params: CodeLensParams) -> anyhow::Result<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];
    
    get_ast(db, file).iter()
        .try_for_each(|node| {
            dispatch!(node.lower(),
            [
                NamespaceDecl => code_lens(db, file, &mut results)
            ]
        );
        anyhow::Ok(())
    })?;
    Ok(Some(results))
}


pub trait GetCodeLens {
    fn code_lens(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<CodeLens>) -> anyhow::Result<()>;
}


impl GetCodeLens for ast::generated::NamespaceDecl {
    fn code_lens(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<CodeLens>) -> anyhow::Result<()> {
        let path = namespace_solver(db, file, self);
        let count = namespace_path(db, path).len();
        results.push(CodeLens {
            range: self.get_lsp_range(),
            command: Some(Command::new("References".into(), "textDocument/completion".into(), None)),
            data: Some(serde_json::json!({
                "count": count,
            })),
        });
        Ok(())
    }
}