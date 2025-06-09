use std::sync::LazyLock;

use ast::generated::{ClassDecl, FbDecl, FuncDecl, Identifier, NamespaceDecl};
use auto_lsp::{anyhow, core::dispatch_once, default::db::{tracked::get_ast, BaseDatabase, File}, lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind}, tree_sitter::{self, Point}};
use auto_lsp::core::ast::AstNode;
use db::namespaces::namespace_solver;

pub fn hover(db: &impl BaseDatabase, params: HoverParams) -> anyhow::Result<Option<Hover>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let document = file.document(db);

    let position = document
        .offset_at(params.text_document_position_params.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position_params.position
            )
        })?;

    if let Some(node) = get_ast(db, file).descendant_at(position) {
        dispatch_once!(node.lower(),
            [
                Identifier => hover(db, file)
            ]
        );
    }
    Ok(None)
}

pub static COMMENT_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        "(comment) @comment",
    )
    .unwrap()
});

pub trait GetHover {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>>;
}


impl GetHover for ast::generated::Identifier {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>> {
        let ast = get_ast(db, file);
        let mut node = self.get_parent(ast);
        while let Some(parent) = node {
            dispatch_once!(parent.lower(),
                [
                    NamespaceDecl => hover(db, file),
                    FuncDecl => hover(db, file),
                    FbDecl => hover(db, file),
                    ClassDecl => hover(db, file)
                ]
            );
            node = parent.get_parent(ast);
        }
        Ok(None)
    }
}

impl GetHover for ast::generated::NamespaceDecl {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>> {
        let path = namespace_solver(db, file, self);
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(r#"```typescript               
namespace {}
"#, path.display(db)),
            }),
            range: Some(self.get_lsp_range()),
        }))
    }
}


impl GetHover for ast::generated::FuncDecl {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>> {
        let path = namespace_solver(db, file, self);
        let doc = file.document(db);
        let text = doc.texter.text.as_bytes();
        let name = self.name.get_text(text)?.to_string();

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(r#"```typescript
namespace {}
function {}
"#, path.display(db), name),
            }),
            range: Some(self.get_lsp_range()),
        }))
    }
}

impl GetHover for ast::generated::FbDecl {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>> {
        let path = namespace_solver(db, file, self);
        let doc = file.document(db);
        let text = doc.texter.text.as_bytes();
        let name = self.name.get_text(text)?.to_string();

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(r#"```typescript
namespace {}
function_block {}
"#, path.display(db), name),
            }),
            range: Some(self.get_lsp_range()),
        }))
    }
}

impl GetHover for ast::generated::ClassDecl {
    fn hover(&self, db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Hover>> {
        let path = namespace_solver(db, file, self);
        let doc = file.document(db);
        let text = doc.texter.text.as_bytes();
        let name = self.name.get_text(text)?.to_string();

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(r#"```typescript
namespace {}
class {}
"#, path.display(db), name),
            }),
            range: Some(self.get_lsp_range()),
        }))
    }
}