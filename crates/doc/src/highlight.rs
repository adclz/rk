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
        // A literal nests captures with the same class (a `numeric_literal`
        // around an `int_literal`); the inner one adds nothing, so it is
        // recorded as `None` and never written.
        let mut open: Vec<Option<usize>> = Vec::new();
        for event in events {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    out.push_str(&escape(&source[start..end]));
                }
                Ok(HighlightEvent::HighlightStart(h)) => {
                    if open.last().copied().flatten() == Some(h.0) {
                        open.push(None);
                        continue;
                    }
                    out.push_str("<span class=\"");
                    out.push_str(&self.classes[h.0]);
                    out.push_str("\">");
                    open.push(Some(h.0));
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    if open.pop().flatten().is_some() {
                        out.push_str("</span>");
                    }
                }
                Err(_) => return escape(source),
            }
        }
        for h in open.iter().rev() {
            if h.is_some() {
                out.push_str("</span>");
            }
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

/// A range of the source to mark, with what to say about it.
pub struct Mark {
    /// Byte offsets into the source the HTML was rendered from.
    pub start: usize,
    pub end: usize,
    /// `error`, `warning` or `info`.
    pub class: String,
    /// The message alone, escaped, for the `title` attribute.
    pub title: String,
    /// The popup's HTML, already escaped.
    pub popup: String,
}

/// Wrap ranges of highlighted HTML in `<mark>` elements without breaking the
/// highlight spans: at every boundary the open spans are closed, the marks
/// adjusted, and the spans reopened. `html` must come from [`StHighlighter::html`]
/// or [`escape`], whose only tags are `<span class="…">`/`</span>` and whose
/// only entities are `&amp;`, `&lt;`, `&gt;`, `&quot;`.
pub fn mark_html(html: &str, marks: &[Mark]) -> String {
    if marks.is_empty() {
        return html.to_string();
    }
    let mut boundaries: Vec<usize> = marks.iter().flat_map(|m| [m.start, m.end]).collect();
    boundaries.sort_unstable();
    boundaries.dedup();

    let open_marks = |at: usize| -> Vec<usize> {
        marks
            .iter()
            .enumerate()
            .filter(|(_, m)| m.start <= at && at < m.end)
            .map(|(i, _)| i)
            .collect()
    };

    let mut out = String::with_capacity(html.len() * 2);
    let mut spans: Vec<&str> = Vec::new(); // open `<span …>` tags, outermost first
    let mut active: Vec<usize> = Vec::new(); // open marks, outermost first
    let mut offset = 0usize; // text offset in the source
    let mut next_boundary = 0usize;
    let mut rest = html;

    let sync = |out: &mut String, spans: &Vec<&str>, active: &mut Vec<usize>, offset: usize| {
        let wanted = open_marks(offset);
        if *active == wanted {
            return;
        }
        for _ in spans {
            out.push_str("</span>");
        }
        for _ in active.iter() {
            out.push_str("</mark>");
        }
        for &i in &wanted {
            let m = &marks[i];
            out.push_str(&format!(
                "<mark class=\"diag {}\" title=\"{}\"><span class=\"diag-popup\">{}</span>",
                m.class, m.title, m.popup
            ));
        }
        for tag in spans {
            out.push_str(tag);
        }
        *active = wanted;
    };

    while !rest.is_empty() {
        if next_boundary < boundaries.len() && boundaries[next_boundary] == offset {
            next_boundary += 1;
            sync(&mut out, &spans, &mut active, offset);
        }
        if let Some(tag_end) = rest.strip_prefix('<').map(|r| r.find('>').unwrap()) {
            let tag = &rest[..tag_end + 2];
            if tag == "</span>" {
                spans.pop();
            } else {
                spans.push(tag);
            }
            out.push_str(tag);
            rest = &rest[tag_end + 2..];
            continue;
        }
        let (piece, consumed) = if rest.starts_with('&') {
            let semi = rest.find(';').unwrap();
            (&rest[..semi + 1], 1)
        } else {
            let ch = rest.chars().next().unwrap();
            (&rest[..ch.len_utf8()], ch.len_utf8())
        };
        out.push_str(piece);
        offset += consumed;
        rest = &rest[piece.len()..];
    }
    // Close whatever a mark left open at the very end, then append the
    // popups: they sit after the code so they never disturb the columns.
    sync(&mut out, &spans, &mut active, offset);
    for _ in &active {
        out.push_str("</mark>");
    }
    out
}
