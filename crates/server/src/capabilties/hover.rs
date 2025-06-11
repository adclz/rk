use std::sync::LazyLock;

use ast::generated::{ClassDecl, FbDecl, FuncDecl, Identifier, NamespaceDecl};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    core::{dispatch_once, document::Document},
    default::db::{tracked::get_ast, BaseDatabase, File},
    lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind},
    tree_sitter::{self, Point},
};
use db::solver::namespace_solver;

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
                value: format!(
                    r#"```typescript               
namespace {}
```
{}
"#,
                    path.display(db), 
                    get_comment(&file.document(db), self.get_lsp_range().start.line as usize).unwrap_or_default()
                ),
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
                value: format!(
                    r#"```typescript
namespace {}
function {}
```
{}                
"#,
                    path.display(db),
                    name,
                    get_comment(&doc, self.get_lsp_range().start.line as usize).unwrap_or_default()
                ),
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
                value: format!(
                    r#"```typescript
namespace {}
function_block {}
```
{}
"#,
                    path.display(db),
                    name,
                    get_comment(&doc, self.get_lsp_range().start.line as usize).unwrap_or_default(),
                ),
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
                value: format!(
                    r#"```typescript
namespace {}
class {}
```
{}
"#,
                    path.display(db),
                    name,
                    get_comment(&doc, self.get_lsp_range().start.line as usize).unwrap_or_default()
                ),
            }),
            range: Some(self.get_lsp_range()),
        }))
    }
}

fn get_comment(doc: &Document, line: usize) -> Option<String> {
    let mut ctr = 1;
    let mut line_above = doc.texter.text.lines().nth(line - ctr)?;

    // Skip empty lines
    while line_above.trim().is_empty() {
        ctr += 1;
        line_above = doc.texter.text.lines().nth(line - ctr)?;
    }

    // Handle single-line comments
    if line_above.trim().starts_with("//") {
        return Some(
            line_above
                .trim()
                .trim_start_matches("//")
                .trim()
                .to_string(),
        );
    }

    // Handle multi-line comments
    let trimmed = line_above.trim();
    let (is_end, start_marker, end_marker) = if trimmed.ends_with("*/") {
        (true, "/*", "*/")
    } else if trimmed.ends_with("*)") {
        (true, "(*", "*)")
    } else {
        return None;
    };

    if !is_end {
        return None;
    }

    let end_line = line - ctr;

    // Check if it's a single-line multi-line comment
    if trimmed.starts_with(start_marker) {
        return Some(
            trimmed
                .trim_start_matches(start_marker)
                .trim_end_matches(end_marker)
                .trim()
                .to_string(),
        );
    }

    // Find the start of the multi-line comment
    ctr += 1;
    while let Some(prev_line) = doc.texter.text.lines().nth(line - ctr) {
        line_above = prev_line;
        if line_above.trim().starts_with(start_marker) {
            break;
        }
        ctr += 1;
    }

    let start_line = line - ctr;

    // Extract all lines between comment markers
    let mut comment = String::new();
    for i in start_line..=end_line {
        if let Some(comment_line) = doc.texter.text.lines().nth(i) {
            let trimmed = comment_line.trim();
            let line_content = if i == start_line {
                trimmed.trim_start_matches(start_marker).trim()
            } else if i == end_line {
                trimmed.trim_end_matches(end_marker).trim()
            } else {
                trimmed
            };

            if !comment.is_empty() {
                comment.push('\n');
            }
            comment.push_str(line_content);
        }
    }

    Some(comment)
}

#[cfg(test)]
mod tests {
    use auto_lsp::texter::core::text::Text;

    use super::*;

    #[test]
    fn single_line_comment() {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();

        let text = r#"// This is a comment
NAMESPACE NS

END_NAMESPACE

// This is another comment


NAMESPACE NS

END_NAMESPACE
"#;
        let tree = p.parse(text, None).unwrap();
        let doc = Document::new(Text::new(text.into()), tree);

        assert_eq!(get_comment(&doc, 1), Some("This is a comment".to_string()));

        assert_eq!(
            get_comment(&doc, 8),
            Some("This is another comment".to_string())
        );
    }

    #[test]
    fn multiline_comment_slash() {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();

        let text = r#"/* This is a comment */
NAMESPACE NS

END_NAMESPACE

/* 
    This is another comment
*/

NAMESPACE NS

END_NAMESPACE
"#;
        let tree = p.parse(text, None).unwrap();
        let doc = Document::new(Text::new(text.into()), tree);
        assert_eq!(get_comment(&doc, 1), Some("This is a comment".to_string()));

        assert_eq!(
            get_comment(&doc, 8),
            Some("This is another comment\n".to_string())
        );
    }

    #[test]
    fn multiline_comment_parenthesis() {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();

        let text = r#"(* This is a comment *)
NAMESPACE NS

END_NAMESPACE

(* 
    This is another comment
*)

NAMESPACE NS

END_NAMESPACE
"#;
        let tree = p.parse(text, None).unwrap();
        let doc = Document::new(Text::new(text.into()), tree);
        assert_eq!(get_comment(&doc, 1), Some("This is a comment".to_string()));

        assert_eq!(
            get_comment(&doc, 8),
            Some("This is another comment\n".to_string())
        );
    }
}
