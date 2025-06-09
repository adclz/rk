use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};
use auto_lsp::tree_sitter::StreamingIterator;
use auto_lsp::{anyhow, tree_sitter};
use std::sync::LazyLock;

static FOLD: &str = r#"
[
  (namespace_decl)
  (data_type_decl)
  (func_decl)
  (fb_decl)
  (class_decl)
  (interface_decl)
] @fold

[
  (comment) @fold.comment
] @fold.comment"#;

pub static FOLD_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), FOLD)
        .expect("Failed to create fold query")
});

/// Request for folding ranges
pub fn folding_ranges(
    db: &impl BaseDatabase,
    params: FoldingRangeParams,
) -> anyhow::Result<Option<Vec<FoldingRange>>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let document = file.document(db);

    let root_node = document.tree.root_node();
    let source = document.texter.text.as_str();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(&FOLD_QUERY, root_node, source.as_bytes());

    let mut ranges = vec![];

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        let kind = match FOLD_QUERY.capture_names()[capture.index as usize] {
            "fold.comment" => FoldingRangeKind::Comment,
            _ => FoldingRangeKind::Region,
        };
        let range = capture.node.range();
        ranges.push(FoldingRange {
            start_line: range.start_point.row as u32,
            start_character: Some(range.start_point.column as u32),
            end_line: range.end_point.row as u32,
            end_character: Some(range.end_point.column as u32),
            kind: Some(kind),
            collapsed_text: None,
        });
    }

    Ok(Some(ranges))
}