//! What a completion item WRITES has to be valid where it is offered.
//!
//! The placeholders are hints the user types over, but they are also what
//! lands in the buffer when the item is accepted and tabbed through. Four of
//! them named a keyword — `class`, `interface`, `method`, `var` — and
//! identifiers are case-insensitive, so accepting `CLASS` wrote a class the
//! compiler then refused.

use db::RootDatabase;
use ide_proto::handlers::completions_utils::static_snippets as snip;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// A snippet's default expansion: `${1:name}` becomes `name`, and a choice
/// `${1|a,b|}` becomes its first alternative.
fn expand(snippet: &str) -> String {
    let mut out = String::new();
    let mut rest = snippet;
    while let Some(at) = rest.find("${") {
        out.push_str(&rest[..at]);
        let body_end = rest[at..].find('}').expect("a closed placeholder") + at;
        let body = &rest[at + 2..body_end];
        let text = match body.split_once(':') {
            Some((_, default)) => default,
            // A choice: `1|I,Q,M|`
            None => body
                .split_once('|')
                .map(|(_, choices)| choices.trim_end_matches('|').split(',').next().unwrap())
                .unwrap_or(""),
        };
        out.push_str(text);
        rest = &rest[body_end + 1..];
    }
    out.push_str(rest);
    out
}

/// Every snippet that writes a name, expanded and placed where the item is
/// offered. A syntax error means the item writes something the compiler
/// cannot read.
#[rstest]
fn a_snippet_writes_what_the_compiler_reads(mut with_db: RootDatabase) {
    let at_file_level = |item: String| item;
    let in_a_pou = |item: String| format!("FUNCTION_BLOCK holder\n{item}\nEND_FUNCTION_BLOCK\n");
    let in_a_body = |item: String| format!("FUNCTION holder : INT\n{item}\nEND_FUNCTION\n");
    let in_a_config = |item: String| format!("CONFIGURATION holder\n{item}\nEND_CONFIGURATION\n");

    let cases: Vec<(&str, String)> = vec![
        (
            "NAMESPACE",
            at_file_level(expand(&insert(snip::namespace()))),
        ),
        ("FUNCTION", at_file_level(expand(&insert(snip::function())))),
        (
            "FUNCTION_BLOCK",
            at_file_level(expand(&insert(snip::function_block()))),
        ),
        ("CLASS", at_file_level(expand(&insert(snip::class())))),
        ("PROGRAM", at_file_level(expand(&insert(snip::program())))),
        (
            "INTERFACE",
            at_file_level(expand(&insert(snip::interface()))),
        ),
        ("TYPE", at_file_level(expand(&insert(snip::type_())))),
        (
            "CONFIGURATION",
            at_file_level(expand(&insert(snip::configuration()))),
        ),
        ("METHOD", in_a_pou(expand(&insert(snip::method())))),
        ("VAR", in_a_pou(expand(&insert(snip::var())))),
        ("VAR_INPUT", in_a_pou(expand(&insert(snip::var_input())))),
        ("VAR_OUTPUT", in_a_pou(expand(&insert(snip::var_output())))),
        ("VAR_IN_OUT", in_a_pou(expand(&insert(snip::var_in_out())))),
        ("VAR_TEMP", in_a_pou(expand(&insert(snip::var_temp())))),
        ("IF", in_a_body(expand(&insert(snip::if_())))),
        ("FOR", in_a_body(expand(&insert(snip::for_())))),
        ("WHILE", in_a_body(expand(&insert(snip::while_())))),
        ("REPEAT", in_a_body(expand(&insert(snip::repeat())))),
        ("RESOURCE", in_a_config(expand(&insert(snip::resource())))),
        (
            "VAR_GLOBAL",
            in_a_config(expand(&insert(snip::var_global()))),
        ),
        (
            "STRUCT",
            in_a_pou(format!(
                "VAR\n\tx : {};\nEND_VAR",
                expand(&insert(snip::struct_()))
            )),
        ),
        (
            "ARRAY",
            in_a_pou(format!(
                "VAR\n\tx : {};\nEND_VAR",
                expand(&insert(snip::array()))
            )),
        ),
    ];

    for (label, source) in cases {
        let mut db = with_db.clone();
        let rendered = test_diagnostics(&mut db, &[&source]);
        let syntax: Vec<&str> = rendered
            .lines()
            .filter(|line| line.starts_with("[E00") || line.starts_with("[E0050"))
            .collect();

        assert!(
            syntax.is_empty(),
            "the {label} snippet writes a syntax error: {syntax:?}\n{source}"
        );
    }
}

fn insert(item: auto_lsp::lsp_types::CompletionItem) -> String {
    item.insert_text.expect("a snippet")
}
