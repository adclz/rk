// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};
use auto_lsp::tree_sitter::StreamingIterator;
use auto_lsp::{anyhow, tree_sitter};
use db::WorkspaceDataBase;
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
    db: &impl WorkspaceDataBase,
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
        // Converted like every range sent to the editor: its lines are the
        // document's, and its columns in the client's encoding. The
        // parser's own rows and byte columns went out as they were.
        let Some(range) = hir::denormalize_in(document, &capture.node.range()) else {
            continue;
        };
        ranges.push(FoldingRange {
            start_line: range.start.line,
            start_character: Some(range.start.character),
            end_line: range.end.line,
            end_character: Some(range.end.character),
            kind: Some(kind),
            collapsed_text: None,
        });
    }

    Ok(Some(ranges))
}

#[cfg(test)]
mod tests {
    use auto_lsp::default::db::file::File;
    use auto_lsp::lsp_types::{
        PartialResultParams, PositionEncodingKind, TextDocumentIdentifier, Url,
        WorkDoneProgressParams,
    };
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

    /// A folding range is sent on the lines an editor counts and in its
    /// encoding, like every other range. The parser's own rows and byte
    /// columns went out: a line short past a carriage return no `\n`
    /// follows, and bytes where UTF-16 units were asked.
    #[test]
    fn a_folding_range_is_on_the_editor_s_lines() {
        let source =
            "(* a *)\r(* 🎈 *) FUNCTION_BLOCK fb\nVAR\n    x : INT;\nEND_VAR\nEND_FUNCTION_BLOCK";
        let url = Url::parse("file:///fold.st").unwrap();
        let mut db = db::RootDatabase::default();
        let file = File::from_string()
            .db(&db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source.to_string())
            .encoding(&PositionEncodingKind::UTF16)
            .call()
            .unwrap();
        db.add_file(file).unwrap();

        let ranges = folding_ranges(
            &db,
            FoldingRangeParams {
                text_document: TextDocumentIdentifier { uri: url },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            },
        )
        .unwrap()
        .unwrap();
        // The block: from `FUNCTION_BLOCK`, after the balloon's two units.
        let block = ranges
            .iter()
            .find(|range| range.end_line == 5)
            .expect("the block folds");
        assert_eq!((block.start_line, block.start_character), (1, Some(9)));
        // The VAR section, two lines below the block's first.
        assert!(ranges.iter().any(|r| (r.start_line, r.end_line) == (2, 4)));
    }
}
