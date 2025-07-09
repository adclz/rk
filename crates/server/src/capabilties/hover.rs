use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    core::{dispatch_once, document::Document},
    default::db::{tracked::get_ast, BaseDatabase, file::File},
    lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind},
};
use db::hir::expression::{Expr, ExprKind, PrimaryExpr};
use db::solver::namespace::{namespaces_in_file};
use db::to_proto::{Extends, IterToProto};

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

    let ns = namespaces_in_file(db, file).unwrap();
    Ok(ns.named_descendant_at(db, position).and_then(|symbol| {
        let ns = ns.namespace_at(db, position)?;
        let symbol = match symbol.symbol_info(db) {
            Some(symbol) => symbol,
            None => return None,
        };

        let namespace = if symbol.kind == Some(auto_lsp::lsp_types::SymbolKind::NAMESPACE) {
            String::default()
        } else {
            format!("namespace {}\n", ns.path(db).to_string(db))
        }; 

        let kind = symbol.kind_to_string();

        let spec = if symbol.spec.is_some() {
            format!("{}: {}", symbol.name, symbol.spec_to_string(db))
        } else {
            symbol.name
        };

        let comment = get_comment(
            &file.document(db),
            symbol.range.lsp().start.line as usize,
        )
        .unwrap_or_default();

        let implements = if let Some(implements) = symbol.implements {
            format!(" implements {}", implements.iter().map(|i| match i.expr(db) {
                ExprKind::PrimaryExpr{ expr: PrimaryExpr::Target(target) } => target.namespace(db).to_string(db),
                _ => "unknown".into(), 
            }).collect::<Vec<_>>().join(", "))
        } else {
            String::default()
        };

        let extends = if let Some(extends) = symbol.extends {
            match extends {
                Extends::Single(extends) => format!(" extends {}", match extends.expr(db) {
                    ExprKind::PrimaryExpr{ expr: PrimaryExpr::Target(target) } => target.namespace(db).to_string(db),
                    _ => "unknown".into(),
                }),
                Extends::Multiple(extends) => format!(" extends {}", extends.iter().map(|i| match i.expr(db) {
                    ExprKind::PrimaryExpr{ expr: PrimaryExpr::Target(target) } => target.namespace(db).to_string(db),
                    _ => "unknown".into(),
                }).collect::<Vec<_>>().join(", ")),
            }
        } else {
            String::default()
        };
         
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"```typescript
{namespace}{kind} {spec}{implements}{extends}
```
{comment}
"#,
                ),
            }),
            range: Some(symbol.range.into()),
        })
    }))
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
    use auto_lsp::{lsp_types::PositionEncodingKind, texter::core::text::Text, tree_sitter};

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
        let doc = Document::new(text.into(), tree, None);

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
        let doc = Document::new(text.into(), tree, None);
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
        let doc = Document::new(text.into(), tree, None);
        assert_eq!(get_comment(&doc, 1), Some("This is a comment".to_string()));

        assert_eq!(
            get_comment(&doc, 8),
            Some("This is another comment\n".to_string())
        );
    }
}
