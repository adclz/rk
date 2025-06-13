use ast::generated::{ClassDecl, FbDecl, FuncDecl, NamespaceDecl};
use auto_lsp::{anyhow, core::dispatch, default::db::{tracked::get_ast, BaseDatabase, File}, lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams}};
use auto_lsp::core::ast::AstNode;
use db::solver::{namespace_solver};

pub fn inlay_hints(db: &impl BaseDatabase, params: InlayHintParams) -> anyhow::Result<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    for node in get_ast(db, file).iter() {
        if node.get_lsp_range().end > range.end {
            break;
        };
            dispatch!(node.lower(),
            [
                NamespaceDecl => inlay_hints(db, file, &mut results),
                FuncDecl => inlay_hints(db, file, &mut results),
                FbDecl => inlay_hints(db, file, &mut results),
                ClassDecl => inlay_hints(db, file, &mut results)
            ]
        );
    }
    Ok(Some(results))
}


pub trait GetInlayHints {
    fn inlay_hints(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<InlayHint>) -> anyhow::Result<()>;
}


impl GetInlayHints for ast::generated::NamespaceDecl {
    fn inlay_hints(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<InlayHint>) -> anyhow::Result<()> {
        let path = namespace_solver(db, file, self);
        if self.internal.is_some() {
            results.push(InlayHint {
                label: InlayHintLabel::String(format!("[internal] namespace {}", path.display(db))),
                position: self.get_lsp_range().end,
                kind: Some(InlayHintKind::TYPE),   
                text_edits: None,
                padding_left: Some(true),
                padding_right: None, 
                data: None,
                tooltip: None,
             });
        } else {
            results.push(InlayHint {
                label: InlayHintLabel::String(format!("namespace {}", path.display(db))),
                position: self.get_lsp_range().end,
                kind: Some(InlayHintKind::TYPE),    
                text_edits: None,
                padding_left: Some(true),
                padding_right: None, 
                data: None,
                tooltip: None,
             });
        }
        Ok(())
    }
}

impl GetInlayHints for ast::generated::FuncDecl {
    fn inlay_hints(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<InlayHint>) -> anyhow::Result<()> {
        let doc = file.document(db);
        let name = self.name.get_text(doc.texter.text.as_bytes())?;
            results.push(InlayHint {
                label: InlayHintLabel::String(format!("fn {name}")),
                position: self.get_lsp_range().end,
                kind: Some(InlayHintKind::TYPE),   
                text_edits: None,
                padding_left: Some(true),
                padding_right: None, 
                data: None,
                tooltip: None,
             });
        Ok(())
    }
}

impl GetInlayHints for ast::generated::FbDecl {
    fn inlay_hints(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<InlayHint>) -> anyhow::Result<()> {
        let doc = file.document(db);
        let name = self.name.get_text(doc.texter.text.as_bytes())?;
            results.push(InlayHint {
                label: InlayHintLabel::String(format!("fn_block {name}")),
                position: self.get_lsp_range().end,
                kind: Some(InlayHintKind::TYPE),   
                text_edits: None,
                padding_left: Some(true),
                padding_right: None, 
                data: None,
                tooltip: None,
             });
        Ok(())
    }
}

impl GetInlayHints for ast::generated::ClassDecl {
    fn inlay_hints(&self, db: &impl BaseDatabase, file: File, results: &mut Vec<InlayHint>) -> anyhow::Result<()> {
        let doc = file.document(db);
        let name = self.name.get_text(doc.texter.text.as_bytes())?;
            results.push(InlayHint {
                label: InlayHintLabel::String(format!("class {name}")),
                position: self.get_lsp_range().end,
                kind: Some(InlayHintKind::TYPE),   
                text_edits: None,
                padding_left: Some(true),
                padding_right: None, 
                data: None,
                tooltip: None,
             });
        Ok(())
    }
}