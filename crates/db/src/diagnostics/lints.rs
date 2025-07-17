use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::DiagnosticRelatedInformation,
    tree_sitter::{self, StreamingIterator},
};
use salsa::Accumulator;
use std::sync::LazyLock;

use crate::diagnostics::{diagnostic_builder::diag, DiagnosticAccumulator};

// Combined query for all linting rules
static LINTS_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        r#"
; Duplicate namespace declarations
( 
 (namespace_decl name: (_) @dup_ns1) 
 (namespace_decl name: (_) @dup_ns2) 
 (#eq? @dup_ns1 @dup_ns2)
)

; Duplicate declarations in namespaces
( 
 [
    (func_decl name: (_) @dup_decl1)
    (fb_decl name: (_) @dup_decl1)
    (class_decl name: (_) @dup_decl1)
 ] 
 [
    (func_decl name: (_) @dup_decl2)
    (fb_decl name: (_) @dup_decl2)
    (class_decl name: (_) @dup_decl2)
 ] 
 (#eq? @dup_decl1 @dup_decl2)
)
"#,
    )
    .expect("Failed to create lints query")
});

/// Executed per file
#[salsa::tracked]
pub fn get_duplicates_by_query(db: &dyn BaseDatabase, file: File) {
    let doc = file.document(db);
    let root_node = doc.tree.root_node();
    let source = doc.texter.text.as_str();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(&LINTS_QUERY, root_node, source.as_bytes());

    // Track state for each lint type
    let mut dup_ns1 = None;
    let mut dup_decl1 = None;

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        let capture_name = LINTS_QUERY.capture_names()[capture.index as usize];

        match capture_name {
            // Duplicate namespace declarations
            "dup_ns1" => {
                dup_ns1 = Some(capture.node.range());
            }
            "dup_ns2" => {
                if let Some(range1) = dup_ns1 {
                    handle_duplicate_namespace(db, file, source, range1, capture.node);
                }
                dup_ns1 = None;
            }

            // Duplicate declarations in namespaces
            "dup_decl1" => {
                dup_decl1 = Some(capture.node.range());
            }
            "dup_decl2" => {
                if let Some(range1) = dup_decl1 {
                    handle_duplicate_declaration(db, file, source, range1, capture.node);
                }
                dup_decl1 = None;
            }

            _ => {}
        }
    }
}

fn handle_duplicate_namespace(
    db: &dyn BaseDatabase,
    file: File,
    source: &str,
    first_range: tree_sitter::Range,
    duplicate_node: tree_sitter::Node,
) {
    let range = Span::from(duplicate_node.range());
    let name = duplicate_node.utf8_text(source.as_bytes()).unwrap();

    DiagnosticAccumulator::accumulate(
        diag()
            .range(range.into())
            .message(format!(
                "duplicate declarations of namespace '{name}' in same scope"
            ))
            .source("IEC".into())
            .severity(auto_lsp::lsp_types::DiagnosticSeverity::INFORMATION)
            .related_information(vec![DiagnosticRelatedInformation {
                location: auto_lsp::lsp_types::Location {
                    uri: file.url(db).clone(),
                    range: Span::from(first_range).into(),
                },
                message: format!("'{name}' is previously declared here"),
            }])
            .call()
            .into(),
        db,
    );
}

fn handle_duplicate_declaration(
    db: &dyn BaseDatabase,
    file: File,
    source: &str,
    first_range: tree_sitter::Range,
    duplicate_node: tree_sitter::Node,
) {
    let range = Span::from(duplicate_node.range());
    let name = duplicate_node.utf8_text(source.as_bytes()).unwrap();

    DiagnosticAccumulator::accumulate(
        diag()
            .range(range.into())
            .message(format!(
                "duplicate declarations of '{name}' in same namespace"
            ))
            .source("IEC".into())
            .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
            .related_information(vec![DiagnosticRelatedInformation {
                location: auto_lsp::lsp_types::Location {
                    uri: file.url(db).clone(),
                    range: Span::from(first_range).into(),
                },
                message: format!("'{name}' is previously declared here"),
            }])
            .call()
            .into(),
        db,
    );
}

#[cfg(test)]
mod tests {
    use crate::{diagnostics::cached_diagnostics, RootDatabase};
    use auto_lsp::{
        default::db::{file::File, BaseDatabase, FileManager},
        lsp_types::{self, DiagnosticSeverity},
    };

    #[test]
    fn duplicate_namespace() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns 

END_NAMESPACE

NAMESPACE ns    

END_NAMESPACE
"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();

        let diagnostics = cached_diagnostics(&db, file);
        assert_eq!(diagnostics.len(), 1);

        let diagnostic = diagnostics[0].diagnostic.clone();
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::INFORMATION));
        assert_eq!(
            diagnostic.message,
            "duplicate declarations of namespace 'ns' in same scope"
        );
    }

    #[test]
    fn duplicate_declaration() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns 

    FUNCTION f

    END_FUNCTION

    FUNCTION f

    END_FUNCTION

END_NAMESPACE
"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();

        let diagnostics = cached_diagnostics(&db, file);
        assert_eq!(diagnostics.len(), 1);

        let diagnostic = diagnostics[0].diagnostic.clone();
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostic.message,
            "duplicate declarations of 'f' in same namespace"
        );
    }
}
