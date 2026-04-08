use std::sync::LazyLock;

use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
    lsp_types::DiagnosticSeverity,
    tree_sitter::{self, StreamingIterator},
};
use db::config_file::LinterConfig;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "duplicate-var-section";

/// L0103: duplicate variable section in the same POU.
struct DuplicateVarSection;

impl ErrorCode for DuplicateVarSection {
    fn code(&self) -> &'static str {
        "L0103"
    }

    fn description(&self) -> &'static str {
        "duplicate variable section"
    }
}

static POU_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        "[
            (func_decl)
            (fb_decl)
            (class_decl)
            (prog_decl)
            (method_decl)
        ] @pou",
    )
    .expect("Failed to create POU query")
});

/// Maps tree-sitter node kinds to their human-readable section names.
fn section_name(kind: &str) -> Option<&'static str> {
    match kind {
        "var_decls" => Some("VAR"),
        "input_decls" | "fb_input_decls" => Some("VAR_INPUT"),
        "output_decls" | "fb_output_decls" => Some("VAR_OUTPUT"),
        "in_out_decls" => Some("VAR_IN_OUT"),
        "temp_var_decls" => Some("VAR_TEMP"),
        "external_var_decls" => Some("VAR_EXTERNAL"),
        "retain_var_decls" => Some("VAR RETAIN"),
        "no_retain_var_decls" => Some("VAR NON_RETAIN"),
        "loc_var_decls" => Some("VAR_LOCATED"),
        "global_var_decls" => Some("VAR_GLOBAL"),
        "prog_access_decls" => Some("VAR_ACCESS"),
        _ => None,
    }
}

pub fn check(
    db: &dyn BaseDatabase,
    file: File,
    config: &LinterConfig,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if !config.is_enabled(NAME) {
        return;
    }

    let document = file.document(db);
    let tree = &document.tree;
    let root = tree.root_node();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(&POU_QUERY, root, document.as_bytes());

    while let Some((capture, capture_index)) = captures.next() {
        let pou_node = capture.captures[*capture_index].node;
        check_pou_node(pou_node, diagnostics);
    }
}

/// Returns a dedup key that distinguishes sections with different qualifiers
/// (e.g. `VAR` vs `VAR CONSTANT`).
fn section_key(child: &tree_sitter::Node) -> String {
    let kind = child.kind();
    // Check for CONSTANT qualifier in var_decls, external_var_decls, etc.
    let has_constant = child
        .child_by_field_name("constant")
        .or_else(|| child.child_by_field_name("constant_or_retain"))
        .is_some_and(|n| n.kind() == "CONSTANT");
    if has_constant {
        format!("{kind}__CONSTANT")
    } else {
        kind.to_string()
    }
}

/// Returns the display name for a variable section, including CONSTANT if present.
fn section_display_name(child: &tree_sitter::Node) -> Option<String> {
    let base = section_name(child.kind())?;
    let has_constant = child
        .child_by_field_name("constant")
        .or_else(|| child.child_by_field_name("constant_or_retain"))
        .is_some_and(|n| n.kind() == "CONSTANT");
    if has_constant {
        Some(format!("{base} CONSTANT"))
    } else {
        Some(base.to_string())
    }
}

fn check_pou_node(pou_node: tree_sitter::Node, diagnostics: &mut Vec<IdeDiagnostic>) {
    // Track: section key → range of the first occurrence
    let mut seen: FxHashMap<String, tree_sitter::Range> = FxHashMap::default();

    let mut cursor = pou_node.walk();
    for child in pou_node.named_children(&mut cursor) {
        let Some(name) = section_display_name(&child) else {
            continue;
        };
        let key = section_key(&child);

        if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(key) {
            e.insert(child.range());
        } else {
            let range = child.range();
            let mut d = diag()
                .message(format!("duplicate {name} section"))
                .severity(DiagnosticSeverity::INFORMATION)
                .desc(&DuplicateVarSection)
                .range(Span::from(range))
                .call();

            d.with_note("merge this section with the existing one above".to_string());

            diagnostics.push(d);
        }
    }
}
