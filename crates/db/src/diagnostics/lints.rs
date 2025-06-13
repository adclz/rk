use std::sync::LazyLock;
use salsa::Accumulator;
use auto_lsp::{default::db::{BaseDatabase, File}, lsp_types::DiagnosticRelatedInformation, tree_sitter::{self, StreamingIterator}};

#[salsa::accumulator]
pub struct LintAccumulator(pub auto_lsp::lsp_types::Diagnostic);

impl From<auto_lsp::lsp_types::Diagnostic> for LintAccumulator {
    fn from(d: auto_lsp::lsp_types::Diagnostic) -> Self {
        LintAccumulator(d)
    }
}

impl From<&LintAccumulator> for auto_lsp::lsp_types::Diagnostic {
    fn from(error: &LintAccumulator) -> Self {
        error.0.clone()
    }
}

static UNMERGED_USING_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(),
r#"
((using_directive) . (using_directive)+ @duplicates) @unmerged_using"#)
        .expect("Failed to create unmerged using query")
});

static UNMERGED_NAMESPACE_DECLARATION_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(),
r#"
( 
 (namespace_decl name: (_) @dup1) 
 (namespace_decl name: (_) @dup2) 
 (#eq? @dup1 @dup2)
) "#)
        .expect("Failed to create namespace declaration query")
});

pub fn query_lints(db: &dyn BaseDatabase, file: File) {
    let doc = file.document(db);
    let root_node = doc.tree.root_node();
    let source = doc.texter.text.as_str();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(&UNMERGED_USING_QUERY, root_node, source.as_bytes());

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        if UNMERGED_USING_QUERY.capture_names()[capture.index as usize] == "duplicates" {
            let range = capture.node.range();
            LintAccumulator::accumulate(auto_lsp::lsp_types::Diagnostic {
                range: auto_lsp::lsp_types::Range {
                    start: auto_lsp::lsp_types::Position::new(range.start_point.row as u32, range.start_point.column as u32),
                    end: auto_lsp::lsp_types::Position::new(range.end_point.row as u32, range.end_point.column as u32
                        ),
                },
                code_description: None,
                severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::INFORMATION),
                code: None,
                source: Some("IEC".into()),
                message: "using directives can be merged".into(),
                related_information: None,
                tags: None,
                data: None,
            }.into(), db)
        }
    }

    let mut captures = query_cursor.captures(&UNMERGED_NAMESPACE_DECLARATION_QUERY, root_node, source.as_bytes());
    let mut dup1 = None;
    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        if UNMERGED_NAMESPACE_DECLARATION_QUERY.capture_names()[capture.index as usize] == "dup1" {
            let range = capture.node.range();
            dup1 = Some(range);
        } else if UNMERGED_NAMESPACE_DECLARATION_QUERY.capture_names()[capture.index as usize] == "dup2" {
            if let Some(dup1) = dup1 {
                let range = capture.node.range();
                let name = capture.node.utf8_text(source.as_bytes()).unwrap();  
                LintAccumulator::accumulate(auto_lsp::lsp_types::Diagnostic {
                    range: auto_lsp::lsp_types::Range {
                        start: auto_lsp::lsp_types::Position::new(range.start_point.row as u32, range.start_point.column as u32),
                        end: auto_lsp::lsp_types::Position::new(range.end_point.row as u32, range.end_point.column as u32
                        ),
                    },
                    code_description: None,
                    severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING),
                    code: None,
                    source: Some("IEC".into()),
                    message: format!("duplicate declarations of namespace '{name}' in same scope"),
                    related_information: Some(vec![DiagnosticRelatedInformation {
                        location: auto_lsp::lsp_types::Location {
                            uri: file.url(db).clone(),
                            range: auto_lsp::lsp_types::Range {
                                start: auto_lsp::lsp_types::Position::new(dup1.start_point.row as u32, dup1.start_point.column as u32), 
                                end: auto_lsp::lsp_types::Position::new(dup1.end_point.row as u32, dup1.end_point.column as u32
                                ),
                            },
                        },
                        message: format!("'{name}' is previously declared here"),
                    }]),
                    tags: None,
                    data: None,
                }.into(), db);
            }
            dup1 = None;
        }
    }
}