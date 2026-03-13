use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::NumberOrString;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use ide_proto::handlers::completions_utils::static_snippets::elem_type_names_init;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

fn is_parse_error(code: &Option<NumberOrString>) -> bool {
    match code {
        Some(NumberOrString::String(s)) => s.starts_with("E00"),
        _ => false,
    }
}

/// Verify that every `elem_type_names_init()` snippet produces valid IEC 61131-3 syntax
/// when used in an assignment context.
#[rstest]
fn elem_type_init_snippets_are_valid_syntax(mut with_db: RootDatabase) {
    let items = elem_type_names_init();
    assert!(!items.is_empty(), "should have at least one snippet");

    let sources: Vec<String> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let insert_text = item.insert_text.as_ref()?;
            Some(format!(
                "FUNCTION fn{i}\nVAR\n    x : INT;\nEND_VAR\n    x := {insert_text};\nEND_FUNCTION\n"
            ))
        })
        .collect();

    let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
    add_sources(&mut with_db, &source_refs);

    let mut failures = Vec::new();
    for (file, item) in with_db.get_files().iter().zip(
        items
            .iter()
            .filter(|item| item.insert_text.is_some()),
    ) {
        let diagnostics = diagnostics_for_file(&with_db, *file);
        let parse_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| is_parse_error(&d.diagnostic.code))
            .collect();

        if !parse_errors.is_empty() {
            failures.push(format!(
                "{}: insert_text '{}' produces parse error: {}",
                item.label,
                item.insert_text.as_ref().unwrap(),
                parse_errors
                    .iter()
                    .map(|d| d.diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "The following snippets produce invalid syntax:\n{}",
        failures.join("\n")
    );
}

/// Verify that every `elem_type_names_init()` snippet produces valid syntax
/// when used in a variable declaration initializer context.
#[rstest]
fn elem_type_init_snippets_valid_in_var_decl(mut with_db: RootDatabase) {
    let items = elem_type_names_init();

    let sources: Vec<String> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let insert_text = item.insert_text.as_ref()?;
            Some(format!(
                "FUNCTION fn{i}\nVAR\n    x : INT := {insert_text};\nEND_VAR\nEND_FUNCTION\n"
            ))
        })
        .collect();

    let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
    add_sources(&mut with_db, &source_refs);

    let mut failures = Vec::new();
    for (file, item) in with_db.get_files().iter().zip(
        items
            .iter()
            .filter(|item| item.insert_text.is_some()),
    ) {
        let diagnostics = diagnostics_for_file(&with_db, *file);
        let parse_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| is_parse_error(&d.diagnostic.code))
            .collect();

        if !parse_errors.is_empty() {
            failures.push(format!(
                "{}: insert_text '{}' produces parse error in VAR init: {}",
                item.label,
                item.insert_text.as_ref().unwrap(),
                parse_errors
                    .iter()
                    .map(|d| d.diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "The following snippets produce invalid syntax in VAR init:\n{}",
        failures.join("\n")
    );
}
