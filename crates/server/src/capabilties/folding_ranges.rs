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
  (prog_decl)
  (config_decl)
  (resource_decl)

  (input_decls)
  (output_decls)
  (in_out_decls)

  (fb_input_decls)
  (fb_output_decls)
  (temp_var_decls)

  (external_var_decls)
  (var_decls)

  (retain_var_decls)
  (no_retain_var_decls)
  (global_var_decls)

  (access_decls)
  (prog_access_decls)
  (config_init)
] @fold

[ (c_style_comment) (pascal_style_comment) ] @comment
(using_directive) @import
"#;

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

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let document = file.document(db);

    let root_node = document.tree.root_node();
    let source = document.texter.text.as_str();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(&FOLD_QUERY, root_node, source.as_bytes());

    let mut ranges = vec![];

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];

        let kind = match FOLD_QUERY.capture_names()[capture.index as usize] {
            "comment" => FoldingRangeKind::Comment,
            "import" => FoldingRangeKind::Imports,
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

#[cfg(test)]
mod tests {
    use auto_lsp::tree_sitter;

    use super::*;

    #[test]
    fn load_fold_query() {
        tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), FOLD)
            .expect("Failed to create fold query");
    }

    /// Every section a configuration holds folds, VAR_CONFIG and VAR_ACCESS
    /// included, and a PROGRAM's VAR_ACCESS.
    #[test]
    fn configuration_sections_fold() {
        let source = "PROGRAM P
VAR x : INT; END_VAR
VAR_ACCESS
    ax : x : INT READ_ONLY;
END_VAR
END_PROGRAM
CONFIGURATION Cfg
VAR_GLOBAL g : INT; END_VAR
VAR_ACCESS
    acc : Res.P1.x : INT READ_ONLY;
END_VAR
VAR_CONFIG
    Res.P1.x : INT := 3;
END_VAR
    RESOURCE Res ON CPU
    END_RESOURCE
END_CONFIGURATION
";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut captures = cursor.captures(&FOLD_QUERY, tree.root_node(), source.as_bytes());
        let mut folded = vec![];
        while let Some((m, index)) = captures.next() {
            folded.push(m.captures[*index].node.kind());
        }
        for kind in [
            "config_decl",
            "global_var_decls",
            "access_decls",
            "config_init",
            "resource_decl",
            "prog_access_decls",
        ] {
            assert!(folded.contains(&kind), "{kind} does not fold: {folded:?}");
        }
    }
}
