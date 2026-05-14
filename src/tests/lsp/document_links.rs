use ariadne::{Label, Report, ReportKind};
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use ide_proto::handlers::document_links::document_links;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, no_color_and_ascii, sources, with_db};

fn test_url(n: usize) -> Url {
    Url::parse(&format!("file:///test{n}.st")).unwrap()
}

fn render_document_links(db: &RootDatabase, source_texts: &[&str], file_idx: usize) -> String {
    let url = test_url(file_idx);
    let file = db.get_file(&url).unwrap();
    let links = document_links(db, file);

    if links.is_empty() {
        return String::new();
    }

    let file_sources: Vec<(&str, &str)> = source_texts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let url_str = test_url(i);
            // Leak the string so we can use it as a reference
            let leaked: &'static str = Box::leak(url_str.as_str().to_string().into_boxed_str());
            (leaked, *s)
        })
        .collect();

    let source_url = url.as_str();
    let source_text = source_texts[file_idx];

    let mut cache = vec![];

    // Build one report per link
    for link in &links {
        let r = &link.range;
        let start_byte = byte_offset(source_text, r.start.line, r.start.character);
        let end_byte = byte_offset(source_text, r.end.line, r.end.character);
        let target_path = link
            .target
            .as_ref()
            .map(|u| u.path().to_string())
            .unwrap_or_default();

        let msg = format!("-> {}", target_path);

        Report::build(ReportKind::Advice, (source_url, start_byte..end_byte))
            .with_config(no_color_and_ascii())
            .with_message(format!(
                "document link: [{}]",
                link.tooltip.as_deref().unwrap_or("")
            ))
            .with_label(Label::new((source_url, start_byte..end_byte)).with_message(msg))
            .finish()
            .write(sources(file_sources.clone()), &mut cache)
            .unwrap();
    }

    String::from_utf8(cache)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert (line, character) to byte offset in source text.
fn byte_offset(source: &str, line: u32, character: u32) -> usize {
    let mut current_line = 0u32;
    let mut byte_pos = 0;

    for (i, ch) in source.char_indices() {
        if current_line == line
            && (i - byte_pos) as u32 >= character {
                return i;
            }
        if ch == '\n' {
            if current_line == line {
                return i; // character was past end of line
            }
            current_line += 1;
            byte_pos = i + 1;
        }
    }

    source.len()
}

#[rstest]
fn document_link_simple_pou(mut with_db: RootDatabase) {
    let source = r#"
// See [add] for details
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @r"
    Advice: document link: [add]
       ,-[ file:///test0.st:2:9 ]
       |
     2 | // See [add] for details
       |         ^|^
       |          `--- -> /test0.st
    ---'
    ");
}

#[rstest]
fn document_link_cross_file(mut with_db: RootDatabase) {
    let sources = &[
        r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    reset : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#,
        r#"
// Uses [Counter] from another file
FUNCTION main : INT
END_FUNCTION
"#,
    ];
    add_sources(&mut with_db, sources);
    assert_snapshot!(render_document_links(&with_db, sources, 1), @r"
    Advice: document link: [Counter]
       ,-[ file:///test1.st:2:10 ]
       |
     2 | // Uses [Counter] from another file
       |          ^^^|^^^
       |             `----- -> /test0.st
    ---'
    ");
}

#[rstest]
fn document_link_no_brackets(mut with_db: RootDatabase) {
    let source = r#"
// This comment has no brackets
FUNCTION add : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @"");
}

#[rstest]
fn document_link_unresolved(mut with_db: RootDatabase) {
    let source = r#"
// Reference to [NonExistent] type
FUNCTION add : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @"");
}

#[rstest]
fn document_link_multiple(mut with_db: RootDatabase) {
    let source = r#"
// Both [add] and [sub] are defined here
FUNCTION add : INT
END_FUNCTION

FUNCTION sub : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @r"
    Advice: document link: [add]
       ,-[ file:///test0.st:2:10 ]
       |
     2 | // Both [add] and [sub] are defined here
       |          ^|^
       |           `--- -> /test0.st
    ---'
    Advice: document link: [sub]
       ,-[ file:///test0.st:2:20 ]
       |
     2 | // Both [add] and [sub] are defined here
       |                    ^|^
       |                     `--- -> /test0.st
    ---'
    ");
}

#[rstest]
fn document_link_namespace_qualified(mut with_db: RootDatabase) {
    let source = r#"
// See [Util.Helper] for details
NAMESPACE Util
    FUNCTION_BLOCK Helper
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @r"
    Advice: document link: [Util.Helper]
       ,-[ file:///test0.st:2:9 ]
       |
     2 | // See [Util.Helper] for details
       |         ^^^^^|^^^^^
       |              `------- -> /test0.st
    ---'
    ");
}

#[rstest]
fn document_link_c_style_comment(mut with_db: RootDatabase) {
    let source = r#"
/* Reference to [add] */
FUNCTION add : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @r"
    Advice: document link: [add]
       ,-[ file:///test0.st:2:18 ]
       |
     2 | /* Reference to [add] */
       |                  ^|^
       |                   `--- -> /test0.st
    ---'
    ");
}

#[rstest]
fn document_link_pascal_comment(mut with_db: RootDatabase) {
    let source = r#"
(* Reference to [add] *)
FUNCTION add : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(render_document_links(&with_db, &[source], 0), @r"
    Advice: document link: [add]
       ,-[ file:///test0.st:2:18 ]
       |
     2 | (* Reference to [add] *)
       |                  ^|^
       |                   `--- -> /test0.st
    ---'
    ");
}
