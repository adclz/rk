//! `search.json`, what the search field in the top bar looks through: one
//! entry per section of a page, so a result lands on its heading. The field
//! reads `diagnostics.json` beside it for the codes.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
use serde_json::json;

use crate::markdown;

/// Past this, a section's text is cut: enough to match on and to show a line
/// of, not the whole page again.
const TEXT_LIMIT: usize = 600;

pub struct Entry {
    page: String,
    heading: String,
    url: String,
    text: String,
}

/// The sections of one page: the text before its first heading, then one
/// entry per `##`, `###` or `####`. Code blocks are left out, inline code
/// kept: `VAR_CONFIG` in a sentence is what someone searches for.
pub fn entries(title: &str, url: &str, source: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut heading: Option<String> = None;
    let mut reading_heading: Option<String> = None;
    let mut text = String::new();
    let mut in_code = false;
    // The title is the page's, already in each entry.
    let mut in_title = false;
    let mut flush = |heading: &Option<String>, text: &mut String| {
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        text.clear();
        if heading.is_none() && words.is_empty() {
            return;
        }
        let mut words = words;
        if words.len() > TEXT_LIMIT {
            let mut cut = TEXT_LIMIT;
            while !words.is_char_boundary(cut) {
                cut -= 1;
            }
            words.truncate(cut);
        }
        out.push(Entry {
            page: title.to_string(),
            heading: heading.clone().unwrap_or_default(),
            url: match heading {
                Some(h) => format!("{url}#{}", slug(h)),
                None => url.to_string(),
            },
            text: words,
        });
    };
    for event in Parser::new_ext(source, markdown::options()) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                in_code = true
            }
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => in_title = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => in_title = false,
            Event::Start(Tag::Heading { level, .. }) if level != HeadingLevel::H1 => {
                flush(&heading, &mut text);
                reading_heading = Some(String::new());
            }
            Event::End(TagEnd::Heading(level)) if level != HeadingLevel::H1 => {
                heading = reading_heading.take().map(|h| h.trim().to_string());
            }
            Event::Text(t) | Event::Code(t) if !in_code && !in_title => {
                match reading_heading.as_mut() {
                    Some(h) => h.push_str(&t),
                    None => {
                        // `{{ linter_table() }}` and its kin are the generator's,
                        // not the page's words.
                        if !t.contains("{{") {
                            text.push_str(&t);
                        }
                    }
                }
            }
            // A break, or the end of anything: a paragraph, an item, a table
            // cell. Two words never run into one.
            Event::SoftBreak | Event::HardBreak | Event::End(_) => text.push(' '),
            _ => {}
        }
    }
    flush(&heading, &mut text);
    out
}

pub fn to_json(entries: &[Entry]) -> String {
    let items: Vec<_> = entries
        .iter()
        .map(|e| json!({ "p": e.page, "h": e.heading, "u": e.url, "x": e.text }))
        .collect();
    serde_json::to_string(&items).expect("plain strings serialize")
}

/// The id Zola gives a heading: lowercase, each run of anything else than a
/// letter or a digit one `-`, none at either end. `VAR_GLOBAL` is
/// `var-global`.
fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_is_an_entry_that_lands_on_its_heading() {
        let src = "# Configuration\n\nA `PROGRAM` does nothing.\n\n## VAR_GLOBAL\n\nShare a value.\n\n```iecst\nVAR_GLOBAL x : INT; END_VAR\n```\n\n### Wire it to inputs and outputs\n\nA sensor.\n";
        let e = entries("Configuration", "/docs/configuration/", src);
        let got: Vec<_> = e
            .iter()
            .map(|e| (e.heading.as_str(), e.url.as_str(), e.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("", "/docs/configuration/", "A PROGRAM does nothing."),
                (
                    "VAR_GLOBAL",
                    "/docs/configuration/#var-global",
                    "Share a value."
                ),
                (
                    "Wire it to inputs and outputs",
                    "/docs/configuration/#wire-it-to-inputs-and-outputs",
                    "A sensor."
                ),
            ]
        );
    }

    #[test]
    fn the_slug_is_zolas() {
        assert_eq!(slug("SUPER()"), "super");
        assert_eq!(slug("THIS and SUPER"), "this-and-super");
        assert_eq!(
            slug("Keep its values across a power cycle"),
            "keep-its-values-across-a-power-cycle"
        );
    }
}
