use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DocumentLink, Location};
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        interned::{identifier::Ident, namespace::NamespacePath},
        pous::pou::Pou,
    },
    hir_ty::index_graphs::{namespace_pou_index, pou_index},
};

use crate::comment_index::comment_index;

/// A bracket reference found inside a comment, e.g. `[MyFB]` or `[NS.MyType]`.
pub(crate) struct BracketRef {
    /// The text inside the brackets.
    pub(crate) content: String,
    /// Absolute byte offset of the opening bracket `[` in the source.
    pub(crate) open_byte: usize,
    /// Absolute byte offset of the closing bracket `]` in the source (exclusive).
    pub(crate) close_byte: usize,
}

/// Scan a comment's text for `[...]` patterns.
pub(crate) fn find_bracket_refs(text: &str, base_byte: usize) -> Vec<BracketRef> {
    let mut refs = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'[' {
            let start = i;
            i += 1;
            // Find matching close bracket (no nesting)
            while i < bytes.len() && bytes[i] != b']' && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b']' {
                let content = &text[start + 1..i];
                // Only consider valid identifier-like content (letters, digits, underscores, dots)
                if !content.is_empty()
                    && content
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
                    && !content.starts_with('.')
                    && !content.ends_with('.')
                {
                    refs.push(BracketRef {
                        content: content.to_string(),
                        open_byte: base_byte + start,
                        close_byte: base_byte + i + 1,
                    });
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    refs
}

/// Resolve a bracket reference name to a POU.
pub(crate) fn resolve_bracket_ref_to_pou<'db>(
    db: &'db dyn WorkspaceDataBase,
    content: &str,
) -> Option<Pou<'db>> {
    let parts: Vec<&str> = content.split('.').collect();

    match parts.len() {
        1 => {
            let ident = Ident::from_slice(db, content);
            pou_index(db, ident)
        }
        _ => {
            let ns_parts = &parts[..parts.len() - 1];
            let target_name = parts[parts.len() - 1];

            let ns_idents: Vec<Ident> = ns_parts.iter().map(|p| Ident::from_slice(db, p)).collect();
            let ns_path = NamespacePath::new(db, ns_idents);

            let target_ident = Ident::from_slice(db, target_name);
            namespace_pou_index(db, ns_path, target_ident)
        }
    }
}

/// Resolve a bracket reference name to a target location (file URI + range).
pub(crate) fn resolve_bracket_ref(db: &dyn WorkspaceDataBase, content: &str) -> Option<Location> {
    let pou = resolve_bracket_ref_to_pou(db, content)?;
    let file = pou.get_scope_id(db).file(db);
    let range: auto_lsp::lsp_types::Range = pou.get_name_span(db).into();
    Some(Location::new(file.url(db).clone(), range))
}

/// Compute document links for all `[TypeName]` references in comments.
pub fn document_links(db: &dyn WorkspaceDataBase, file: File) -> Vec<DocumentLink> {
    let comment_idx = comment_index(db, file);
    let document = file.document(db);
    let source = document.as_str();

    let mut links = Vec::new();

    for comment in comment_idx.map.values() {
        let text = match source.get(comment.range.start_byte..comment.range.end_byte) {
            Some(t) => t,
            None => continue,
        };

        // Pre-filter: skip comments without square brackets
        if !text.contains('[') {
            continue;
        }

        let bracket_refs = find_bracket_refs(text, comment.range.start_byte);

        for bref in &bracket_refs {
            if let Some(location) = resolve_bracket_ref(db, &bref.content) {
                // Build a tree_sitter::Range for the bracket content (excluding brackets)
                let content_start = bref.open_byte + 1;
                let content_end = bref.close_byte - 1;

                let ts_range = byte_range_to_ts_range(source, content_start, content_end);

                if let Some(enc_range) = document.ts_range_to_enc_range(&ts_range) {
                    let span: Span = enc_range.into();
                    links.push(DocumentLink {
                        range: span.into(),
                        target: Some(location.uri),
                        tooltip: Some(bref.content.clone()),
                        data: None,
                    });
                }
            }
        }
    }

    links
}

/// Convert a byte range in source text to a `tree_sitter::Range`.
pub(crate) fn byte_range_to_ts_range(
    source: &str,
    start_byte: usize,
    end_byte: usize,
) -> tree_sitter::Range {
    let (start_row, start_col) = byte_offset_to_point(source, start_byte);
    let (end_row, end_col) = byte_offset_to_point(source, end_byte);

    tree_sitter::Range {
        start_byte,
        end_byte,
        start_point: tree_sitter::Point {
            row: start_row,
            column: start_col,
        },
        end_point: tree_sitter::Point {
            row: end_row,
            column: end_col,
        },
    }
}

/// Compute (row, column) from a byte offset in source text.
fn byte_offset_to_point(source: &str, offset: usize) -> (usize, usize) {
    let mut row = 0;
    let mut last_newline = 0; // byte position right after the last newline

    for (i, byte) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if byte == b'\n' {
            row += 1;
            last_newline = i + 1;
        }
    }

    (row, offset - last_newline)
}

/// Replace `[TypeName]` bracket references in comment text with markdown links.
///
/// Unresolved references are left as-is (with brackets).
pub(crate) fn replace_bracket_refs_with_links(db: &dyn WorkspaceDataBase, text: &str) -> String {
    if !text.contains('[') {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut last_end = 0;

    while i < bytes.len() {
        if bytes[i] == b'[' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b']' && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b']' {
                let content = &text[start + 1..i];
                if !content.is_empty()
                    && content
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
                    && !content.starts_with('.')
                    && !content.ends_with('.')
                    && let Some(location) = resolve_bracket_ref(db, content) {
                        result.push_str(&text[last_end..start]);
                        result.push_str(&format!("[{}]({})", content, location.uri));
                        i += 1;
                        last_end = i;
                        continue;
                    }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    result.push_str(&text[last_end..]);
    result
}
