//! Syntax highlighting for Structured Text at build time, from the
//! repository's own tree-sitter grammar. The page needs no script: every
//! token is a `<span class="hl-…">` the stylesheet colors.

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Capture names the query uses, in the order their class index is assigned.
/// A capture not in this list is silently unstyled, so keep it in sync with
/// `crates/tree-sitter/queries/highlights.scm`.
const CAPTURES: &[&str] = &[
    "keyword",
    "keyword.storage",
    "keyword.control",
    "keyword.operator",
    "variable.builtin",
    "constant.builtin",
    "type.builtin",
    "comment",
    "attribute",
    "number",
    "string",
    "function",
    "function.method",
    "function.call",
    "type",
    "namespace",
    "variable",
    "operator",
    "punctuation.delimiter",
    "punctuation.bracket",
];

pub struct StHighlighter {
    config: HighlightConfiguration,
    classes: Vec<String>,
}

impl StHighlighter {
    pub fn new() -> Self {
        let mut config = HighlightConfiguration::new(
            tree_sitter_rk::LANGUAGE.into(),
            "iecst",
            tree_sitter_rk::HIGHLIGHTS_QUERY,
            "",
            "",
        )
        .expect("the highlight query must compile against the grammar");
        config.configure(CAPTURES);
        let classes = CAPTURES
            .iter()
            .map(|c| format!("hl-{}", c.replace('.', "-")))
            .collect();
        Self { config, classes }
    }

    /// The source as escaped HTML with highlight spans. Falls back to plain
    /// escaped text if the parser rejects the input outright, so a page with
    /// a deliberately broken example still renders.
    pub fn html(&self, source: &str) -> String {
        let mut highlighter = Highlighter::new();
        let Ok(events) = highlighter.highlight(&self.config, source.as_bytes(), None, |_| None)
        else {
            return escape(source);
        };
        let mut out = String::with_capacity(source.len() * 2);
        let mut open = 0usize;
        for event in events {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    out.push_str(&escape(&source[start..end]));
                }
                Ok(HighlightEvent::HighlightStart(h)) => {
                    out.push_str("<span class=\"");
                    out.push_str(&self.classes[h.0]);
                    out.push_str("\">");
                    open += 1;
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    out.push_str("</span>");
                    open -= 1;
                }
                Err(_) => return escape(source),
            }
        }
        for _ in 0..open {
            out.push_str("</span>");
        }
        out
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
