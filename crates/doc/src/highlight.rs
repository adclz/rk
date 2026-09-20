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
    "constant",
];

pub struct StHighlighter {
    config: HighlightConfiguration,
    classes: Vec<String>,
    /// Word to CSS class for inline `code`, so `VAR_IN_OUT` in a sentence
    /// reads as it does in a block.
    inline_words: std::collections::HashMap<String, String>,
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
        let classes: Vec<String> = CAPTURES
            .iter()
            .map(|c| format!("hl-{}", c.replace('.', "-")))
            .collect();
        Self {
            inline_words: inline_words(&classes),
            config,
            classes,
        }
    }

    /// One inline `code` span, colored. A command line or a flag goes through
    /// the shell tokeniser; anything else is looked up whole, so a span the
    /// table does not know stays plain rather than being guessed at.
    pub fn inline(&self, src: &str) -> String {
        let word = src.trim();
        if word.is_empty() {
            return escape(src);
        }
        if word == "rk" || word.starts_with("rk ") || word.starts_with('-') {
            return shell_html(src);
        }
        if word.len() > 2 && word.starts_with('{') && word.ends_with('}') {
            return span("hl-attribute", src);
        }
        if is_diagnostic_code(word) {
            return span("hl-number", src);
        }
        // `SUPER()` is the same word as `SUPER`.
        let word = word.strip_suffix("()").unwrap_or(word);
        match self.inline_words.get(word) {
            Some(class) => span(class, src),
            None => escape(src),
        }
    }

    /// The source as escaped HTML with highlight spans. Falls back to plain
    /// escaped text if the parser rejects the input outright, so a page with
    /// a deliberately broken example still renders.
    pub fn html(&self, source: &str) -> String {
        self.html_range(source, 0, source.len())
    }

    /// Highlight `source` as though it sat between `prefix` and `suffix`, and
    /// return only the part that was `source`.
    ///
    /// A bare statement or declaration does not parse on its own. Tree-sitter
    /// recovers, but the first token is swallowed into an ERROR node and loses
    /// its capture, which is why a snippet opening on `IF` used to render that
    /// `IF` plain while colouring the one nested inside it. Give the snippet
    /// the POU it was written for and every token is reached.
    pub fn html_in(&self, prefix: &str, source: &str, suffix: &str) -> String {
        let full = format!("{prefix}{source}{suffix}");
        self.html_range(&full, prefix.len(), prefix.len() + source.len())
    }

    /// The bytes of `source` in `lo..hi`, with the spans the whole source
    /// implies: a span open across `lo` is reopened, one still open at `hi`
    /// is closed.
    fn html_range(&self, source: &str, lo: usize, hi: usize) -> String {
        let mut highlighter = Highlighter::new();
        let Ok(events) = highlighter.highlight(&self.config, source.as_bytes(), None, |_| None)
        else {
            return escape(&source[lo..hi]);
        };
        let mut out = String::with_capacity((hi - lo) * 2);
        // The classes the parser has entered, innermost last. A capture that
        // repeats the class already open adds nothing (a `numeric_literal`
        // around an `int_literal` around an `unsigned_int`), so only distinct
        // classes are pushed; `nested` remembers which starts did.
        let mut stack: Vec<usize> = Vec::new();
        let mut nested: Vec<bool> = Vec::new();
        // What is actually written. A span opens only when text needs it, so
        // a capture with no text inside the window leaves no empty tag.
        let mut written: Vec<usize> = Vec::new();
        let sync = |out: &mut String, stack: &[usize], written: &mut Vec<usize>| {
            let common = written
                .iter()
                .zip(stack)
                .take_while(|(a, b)| a == b)
                .count();
            for _ in common..written.len() {
                out.push_str("</span>");
            }
            written.truncate(common);
            for class in &stack[common..] {
                out.push_str("<span class=\"");
                out.push_str(&self.classes[*class]);
                out.push_str("\">");
                written.push(*class);
            }
        };

        for event in events {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    let (start, end) = (start.max(lo), end.min(hi));
                    if start >= end {
                        continue;
                    }
                    sync(&mut out, &stack, &mut written);
                    out.push_str(&escape(&source[start..end]));
                }
                Ok(HighlightEvent::HighlightStart(h)) => {
                    let novel = stack.last() != Some(&h.0);
                    if novel {
                        stack.push(h.0);
                    }
                    nested.push(novel);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    if nested.pop().unwrap_or(false) {
                        stack.pop();
                    }
                }
                Err(_) => return escape(&source[lo..hi]),
            }
        }
        sync(&mut out, &[], &mut written);
        color_brackets(&out)
    }
}

/// VS Code colors bracket pairs by their nesting depth, cycling through three.
/// The grammar cannot say how deep a bracket is, so the depth is counted over
/// the rendered spans, each of which holds exactly one bracket.
fn color_brackets(html: &str) -> String {
    const OPEN: &str = "<span class=\"hl-punctuation-bracket\">";
    if !html.contains(OPEN) {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut depth = 0usize;
    let mut rest = html;
    while let Some(at) = rest.find(OPEN) {
        out.push_str(&rest[..at]);
        let body = at + OPEN.len();
        let Some(close) = rest[body..].find("</span>").map(|n| body + n) else {
            out.push_str(&rest[at..]);
            return out;
        };
        // Adjacent brackets share one span (`)}` ends a wasm pragma), so each
        // character is weighed on its own or the depth drifts.
        for bracket in rest[body..close].chars() {
            if !matches!(bracket, '(' | '[' | '{') {
                depth = depth.saturating_sub(1);
            }
            out.push_str(&format!(
                "<span class=\"hl-bracket-{}\">{bracket}</span>",
                depth % 3 + 1
            ));
            if matches!(bracket, '(' | '[' | '{') {
                depth += 1;
            }
        }
        rest = &rest[close + "</span>".len()..];
    }
    out.push_str(rest);
    out
}

/// The POU a `fragment` snippet is read inside: statements need a body.
pub const FRAGMENT_WRAP: (&str, &str) = ("FUNCTION __Fragment : INT\n", "\nEND_FUNCTION\n");
/// The POU a `decl` snippet is read inside: declarations need a VAR block.
pub const DECL_WRAP: (&str, &str) = (
    "FUNCTION_BLOCK __Decl\nVAR\n",
    "\nEND_VAR\nEND_FUNCTION_BLOCK\n",
);

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

/// A diagnostic code as the compiler and the linter spell them: `E0301`,
/// `L0307`.
fn is_diagnostic_code(word: &str) -> bool {
    let mut chars = word.chars();
    matches!(chars.next(), Some('E' | 'L')) && word.len() >= 4 && chars.all(|c| c.is_ascii_digit())
}

/// Elementary type names, the one part of the theme that cannot be read off
/// the query: it captures them by node name (`(int_type_name)`), and a node
/// name carries no text. IEC 61131-3 table 10, which does not grow.
const TYPE_NAMES: &[&str] = &[
    "BOOL",
    "BYTE",
    "WORD",
    "DWORD",
    "LWORD",
    "SINT",
    "INT",
    "DINT",
    "LINT",
    "USINT",
    "UINT",
    "UDINT",
    "ULINT",
    "REAL",
    "LREAL",
    "TIME",
    "LTIME",
    "DATE",
    "LDATE",
    "TIME_OF_DAY",
    "TOD",
    "LTIME_OF_DAY",
    "LTOD",
    "DATE_AND_TIME",
    "DT",
    "LDATE_AND_TIME",
    "LDT",
    "STRING",
    "WSTRING",
    "CHAR",
    "WCHAR",
];

/// The words the query captures by node name rather than by text, with the
/// capture each one carries.
const NAMED_WORDS: &[(&str, &str)] = &[
    ("NULL", "constant.builtin"),
    ("PRIVATE", "keyword"),
    ("PUBLIC", "keyword"),
    ("PROTECTED", "keyword"),
    ("READ_ONLY", "keyword"),
    ("READ_WRITE", "keyword"),
];

/// Read the highlight query's literal keyword groups, so inline code and
/// fenced code agree by construction: a keyword added to the query colors in
/// both without a second list to edit here.
fn inline_words(classes: &[String]) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let class_of = |capture: &str| {
        CAPTURES
            .iter()
            .position(|c| *c == capture)
            .map(|i| classes[i].clone())
    };

    let src = tree_sitter_rk::HIGHLIGHTS_QUERY;
    let bytes = src.as_bytes();
    let mut pending: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // A comment runs to the end of its line; a `;` inside a string is
            // reached by the arm below first, so it is never mistaken for one.
            b';' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'"' {
                    end += 1;
                }
                pending.push(&src[start..end]);
                i = end + 1;
            }
            b'@' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'.' | b'_'))
                {
                    end += 1;
                }
                if let Some(class) = class_of(&src[start..end]) {
                    for word in pending.drain(..) {
                        // Words only: a lone `.` or `;` in a sentence is
                        // prose, not punctuation to color.
                        if word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
                            out.insert(word.to_string(), class.clone());
                        }
                    }
                }
                pending.clear();
                i = end;
            }
            _ => i += 1,
        }
    }

    if let Some(class) = class_of("type.builtin") {
        for name in TYPE_NAMES {
            out.insert(name.to_string(), class.clone());
        }
    }
    for (word, capture) in NAMED_WORDS {
        if let Some(class) = class_of(capture) {
            out.insert(word.to_string(), class);
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
/// highlight spans: at every boundary the open spans are closed and the marks
/// adjusted, and each span reopens at the next character that needs it.
/// `html` must come from [`StHighlighter::html`] or [`escape`], whose only
/// tags are `<span class="…">`/`</span>` and whose only entities are
/// `&amp;`, `&lt;`, `&gt;`, `&quot;`.
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
    // The spans the input has opened, outermost first, and how many of them
    // are written. A span is emitted only when a character needs it, so a
    // boundary landing exactly on a span's end reopens nothing.
    let mut spans: Vec<&str> = Vec::new();
    let mut written = 0usize;
    let mut active: Vec<usize> = Vec::new(); // open marks, outermost first
    let mut offset = 0usize; // text offset in the source
    let mut next_boundary = 0usize;
    let mut rest = html;

    let sync = |out: &mut String, written: &mut usize, active: &mut Vec<usize>, offset: usize| {
        let wanted = open_marks(offset);
        if *active == wanted {
            return;
        }
        for _ in 0..*written {
            out.push_str("</span>");
        }
        *written = 0;
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
        *active = wanted;
    };

    while !rest.is_empty() {
        if next_boundary < boundaries.len() && boundaries[next_boundary] == offset {
            next_boundary += 1;
            sync(&mut out, &mut written, &mut active, offset);
        }
        if let Some(tag_end) = rest.strip_prefix('<').map(|r| r.find('>').unwrap()) {
            let tag = &rest[..tag_end + 2];
            if tag == "</span>" {
                spans.pop();
                // Close it only if it was ever opened; a `sync` may have
                // closed it already.
                if written > spans.len() {
                    out.push_str("</span>");
                    written -= 1;
                }
            } else {
                spans.push(tag);
            }
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
        while written < spans.len() {
            out.push_str(spans[written]);
            written += 1;
        }
        out.push_str(piece);
        offset += consumed;
        rest = &rest[piece.len()..];
    }
    // Close whatever a mark left open at the very end, then append the
    // popups: they sit after the code so they never disturb the columns.
    sync(&mut out, &mut written, &mut active, offset);
    for _ in &active {
        out.push_str("</mark>");
    }
    out
}

/// TOML, highlighted with the same classes the Structured Text highlighter
/// emits so both read as one theme. Line-based on purpose: the site shows
/// four small configuration blocks, which does not justify a second grammar.
pub fn toml_html(src: &str) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        out.push_str(indent);

        if trimmed.starts_with('#') {
            out.push_str(&span("hl-comment", trimmed));
            continue;
        }
        if trimmed.starts_with('[') {
            // A table header, with any trailing comment kept separate.
            match trimmed.find(']') {
                Some(end) => {
                    out.push_str(&span("hl-type", &trimmed[..=end]));
                    out.push_str(&trailing(&trimmed[end + 1..]));
                }
                None => out.push_str(&escape(trimmed)),
            }
            continue;
        }
        match trimmed.split_once('=') {
            Some((key, rest)) => {
                out.push_str(&span("hl-attribute", key.trim_end()));
                out.push_str(&escape(&key[key.trim_end().len()..]));
                out.push_str(&escape("="));
                out.push_str(&value(rest));
            }
            None => out.push_str(&escape(trimmed)),
        }
    }
    out
}

/// A value and whatever comment follows it.
fn value(rest: &str) -> String {
    let (before, comment) = match rest.find('#') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let body = before.trim();
    let lead = &before[..before.len() - before.trim_start().len()];
    let tail = &before[lead.len() + body.len()..];
    let class = if body.starts_with('"') || body.starts_with('\'') {
        "hl-string"
    } else if body == "true" || body == "false" {
        "hl-constant-builtin"
    } else if body.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        "hl-number"
    } else {
        "hl-attribute"
    };
    format!(
        "{}{}{}{}",
        escape(lead),
        if body.is_empty() {
            String::new()
        } else {
            span(class, body)
        },
        escape(tail),
        trailing(comment)
    )
}

fn trailing(rest: &str) -> String {
    match rest.find('#') {
        Some(i) => format!("{}{}", escape(&rest[..i]), span("hl-comment", &rest[i..])),
        None => escape(rest),
    }
}

fn span(class: &str, text: &str) -> String {
    format!("<span class=\"{class}\">{}</span>", escape(text))
}

/// Shell, in the same theme classes. The site's shell blocks are command
/// lines with flags and comments, so a tokeniser is enough; there is no
/// scripting on any page to justify a grammar.
pub fn shell_html(src: &str) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // A comment may follow a command, so split the line first.
        let (command, comment) = match line.find('#') {
            Some(at) => (&line[..at], &line[at..]),
            None => (line, ""),
        };
        let mut first = true;
        let mut rest = command;
        while !rest.is_empty() {
            let ws = rest.len() - rest.trim_start().len();
            out.push_str(&escape(&rest[..ws]));
            rest = &rest[ws..];
            if rest.is_empty() {
                break;
            }
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let word = &rest[..end];
            let class = if word.starts_with('-') {
                "hl-attribute"
            } else if word.starts_with('\'') || word.starts_with('"') {
                "hl-string"
            } else if first {
                "hl-function"
            } else {
                ""
            };
            if class.is_empty() {
                out.push_str(&escape(word));
            } else {
                out.push_str(&span(class, word));
            }
            first = false;
            rest = &rest[end..];
        }
        if !comment.is_empty() {
            out.push_str(&span("hl-comment", comment));
        }
    }
    out
}

/// A console transcript. Only what a reader scans for is colored: the verdict
/// of each line, and the timings in brackets.
pub fn console_html(src: &str) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // A rule drawn out of box characters is furniture, not output.
        if line.contains('─') && line.chars().all(|c| c == '─' || c == ' ') {
            out.push_str(&span("hl-comment", line));
            continue;
        }
        let mut rest = line;
        while !rest.is_empty() {
            if let Some(bracket) = rest.find('[')
                && let Some(close) = rest[bracket..].find(']')
            {
                out.push_str(&words(&rest[..bracket]));
                out.push_str(&span("hl-number", &rest[bracket..bracket + close + 1]));
                rest = &rest[bracket + close + 1..];
                continue;
            }
            out.push_str(&words(rest));
            break;
        }
    }
    out
}

/// The verdict words of a console line; everything else is left alone.
fn words(text: &str) -> String {
    let mut out = String::new();
    for word in text.split_inclusive(' ') {
        let trimmed = word.trim_end();
        let class = match trimmed {
            "PASS" | "ok" => "hl-pass",
            "FAIL" | "error" | "error:" => "hl-fail",
            _ => "",
        };
        if class.is_empty() {
            out.push_str(&escape(word));
        } else {
            out.push_str(&span(class, trimmed));
            out.push_str(&escape(&word[trimmed.len()..]));
        }
    }
    out
}

/// A shallow tokeniser for the site's few snippets in someone else's
/// configuration language: strings, the key of a `key = value`, and a called
/// name. Enough to read; not a grammar, and not pretending to be one.
pub fn script_html(src: &str) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    let bytes = src.as_bytes();
    let word_char = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':');
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'-' if src[i..].starts_with("--") => {
                let end = src[i..].find('\n').map_or(src.len(), |n| i + n);
                out.push_str(&span("hl-comment", &src[i..end]));
                i = end;
            }
            b'"' | b'\'' => {
                let quote = bytes[i];
                let mut end = i + 1;
                while end < bytes.len() && bytes[end] != quote {
                    end += 1;
                }
                end = (end + 1).min(bytes.len());
                out.push_str(&span("hl-string", &src[i..end]));
                i = end;
            }
            b if word_char(b) && !b.is_ascii_digit() => {
                let mut end = i;
                while end < bytes.len() && word_char(bytes[end]) {
                    end += 1;
                }
                let word = &src[i..end];
                // What follows says what the word is: `=` a key, `(` a call.
                let next = src[end..].trim_start().as_bytes().first().copied();
                let class = match next {
                    Some(b'=') if src[end..].trim_start().as_bytes().get(1) != Some(&b'=') => {
                        "hl-attribute"
                    }
                    Some(b'(') => "hl-function",
                    _ => "",
                };
                if class.is_empty() {
                    out.push_str(&escape(word));
                } else {
                    out.push_str(&span(class, word));
                }
                i = end;
            }
            _ => {
                let ch = src[i..].chars().next().unwrap();
                out.push_str(&escape(&src[i..i + ch.len_utf8()]));
                i += ch.len_utf8();
            }
        }
    }
    out
}
