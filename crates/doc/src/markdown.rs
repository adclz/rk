// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Markdown as the site's sources write it: split the frontmatter, find the
//! fences the compiler must check, and pre-render what Zola cannot, which is
//! `iecst` fences through the grammar's highlighter, inline code with the
//! same classes, and GitHub's `> [!NOTE]` alerts. Everything else stays
//! Markdown for Zola to render.

use std::collections::BTreeMap;
use std::ops::Range;

use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

use crate::highlight::{StHighlighter, escape};

#[derive(Default)]
pub struct Frontmatter {
    pub fields: BTreeMap<String, String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Split `---` frontmatter (the Agent Skills shape) from the body. A file
/// without frontmatter is all body.
pub fn split_frontmatter(text: &str) -> (Frontmatter, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (Frontmatter::default(), text);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (Frontmatter::default(), text);
    };
    let (yaml, body) = (&rest[..end], &rest[end + 5..]);
    let mut fields = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in yaml.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(key) = &current {
                let value = fields.entry(key.clone()).or_insert_with(String::new);
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            fields.insert(key.clone(), value.trim().to_string());
            current = Some(key);
        }
    }
    (Frontmatter { fields }, body)
}

/// Split `+++` frontmatter (Zola's shape) from the body, both verbatim. A
/// file without one is all body.
pub fn split_toml_frontmatter(text: &str) -> (&str, &str) {
    if let Some(rest) = text.strip_prefix("+++\n")
        && let Some(end) = rest.find("\n+++\n")
    {
        return (&rest[..end], &rest[end + 5..]);
    }
    ("", text)
}

/// One fenced code block, with where it sits so a failing example can be
/// named by file and line.
#[derive(Debug, Clone)]
pub struct Fence {
    /// The info string after the backticks, e.g. `iecst expect=E0102`.
    pub info: String,
    pub code: String,
    /// 1-based line of the opening fence.
    pub line: usize,
}

pub struct Preprocessed {
    /// The body with every fence and every inline code span replaced by
    /// its HTML, and the first H1 removed: what Zola renders.
    pub body: String,
    /// The first H1's text, if the body has one.
    pub title: Option<String>,
    /// The `iecst` fences, for the compiler to check.
    pub fences: Vec<Fence>,
}

/// The Markdown GitHub renders. GFM is what parses `> [!NOTE]` as an alert
/// rather than a quote.
pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_GFM
}

/// Pre-render a Markdown body for Zola.
pub fn preprocess(body: &str, highlighter: &StHighlighter) -> Preprocessed {
    let parser = Parser::new_ext(body, options()).into_offset_iter();

    let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
    let mut fences = Vec::new();
    let mut title: Option<String> = None;
    let mut in_fence: Option<(String, usize, Range<usize>, String)> = None;
    let mut h1: Option<(Range<usize>, String)> = None;
    let mut links = 0usize;
    let mut heading: Option<(Range<usize>, String)> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let line = body[..range.start].matches('\n').count() + 1;
                in_fence = Some((info.to_string(), line, range, String::new()));
            }
            Event::Text(text) if in_fence.is_some() => {
                in_fence.as_mut().unwrap().3.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((info, line, range, code)) = in_fence.take() {
                    replacements.push((range, fence_html(&info, &code, highlighter)));
                    if info.split_whitespace().next() == Some("iecst") {
                        fences.push(Fence { info, code, line });
                    }
                }
            }
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) if title.is_none() && h1.is_none() => {
                h1 = Some((range, String::new()));
            }
            Event::Text(text) if h1.is_some() => h1.as_mut().unwrap().1.push_str(&text),
            Event::Code(text) if h1.is_some() => h1.as_mut().unwrap().1.push_str(&text),
            Event::End(TagEnd::Heading(HeadingLevel::H1)) if h1.is_some() => {
                let (range, text) = h1.take().unwrap();
                title = Some(text);
                // The template prints the title; the body must not repeat it.
                replacements.push((range, String::new()));
            }
            Event::Start(Tag::Heading { level, .. }) if level != HeadingLevel::H1 => {
                heading = Some((range, String::new()));
            }
            Event::Text(text) if heading.is_some() => heading.as_mut().unwrap().1.push_str(&text),
            Event::End(TagEnd::Heading(_)) if heading.is_some() => {
                // A heading that is a keyword, `## TASK`, takes the color that
                // part has in the schemas. The class is in Zola's syntax, added
                // to the site's copy: GitHub never sees it.
                let (range, text) = heading.take().unwrap();
                if let Some(kind) = crate::schema::kind_class(text.trim()) {
                    let end = range.start + body[range].trim_end().len();
                    replacements.push((end..end, format!(" {{.{kind}}}")));
                }
            }
            Event::Start(Tag::Link { .. }) => links += 1,
            Event::End(TagEnd::Link) => links -= 1,
            Event::Code(text) => {
                let code = format!("<code>{}</code>", highlighter.inline(&text));
                // A diagnostic code links to its entry, unless the writer
                // already put it in a link of their own.
                let html = match highlighter.code_href(text.trim()) {
                    Some(href) if links == 0 => {
                        format!("<a class=\"code-link\" href=\"{href}\">{code}</a>")
                    }
                    _ => code,
                };
                replacements.push((range, html));
            }
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                alert(&mut replacements, body, range, kind)
            }
            _ => {}
        }
    }

    replacements.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(body.len());
    let mut pos = 0;
    for (range, html) in replacements {
        if range.start < pos {
            continue;
        }
        out.push_str(&body[pos..range.start]);
        out.push_str(&html);
        pos = range.end;
    }
    out.push_str(&body[pos..]);

    Preprocessed {
        body: out,
        title,
        fences,
    }
}

/// A fence as the page shows it: `iecst` through the grammar, the other
/// languages the site uses through their small highlighters. Blank lines
/// on both sides make it an HTML block of its own to the Markdown renderer.
fn fence_html(info: &str, code: &str, highlighter: &StHighlighter) -> String {
    let lang = info.split_whitespace().next().unwrap_or("");
    let code = code.trim_end_matches('\n');
    if lang == "schema" {
        return format!("\n{}\n", crate::schema::html(code));
    }
    let inner = match lang {
        // `st` and `pascal` are what the README and docs/ mark Structured
        // Text as, the second because that is what GitHub highlights it as;
        // here they are the same grammar.
        "iecst" | "pascal" | "st" => {
            // A fragment is highlighted in the same POU the fence gate
            // checks it in, so the two never disagree.
            match info
                .split_whitespace()
                .find(|w| matches!(*w, "fragment" | "decl"))
            {
                Some("fragment") => {
                    let (before, after) = crate::highlight::FRAGMENT_WRAP;
                    highlighter.html_in(before, code, after)
                }
                Some("decl") => {
                    let (before, after) = crate::highlight::DECL_WRAP;
                    highlighter.html_in(before, code, after)
                }
                _ => highlighter.html(code),
            }
        }
        "toml" => crate::highlight::toml_html(code),
        "sh" | "bash" | "shell" => crate::highlight::shell_html(code),
        "console" | "text" => crate::highlight::console_html(code),
        "lua" => crate::highlight::script_html(code),
        _ => escape(code),
    };
    let inner = match lang {
        "sh" | "bash" | "shell" | "console" | "text" => report_codes(&inner, highlighter),
        _ => inner,
    };
    // Box-drawing characters, a compiler diagnostic's frame or a drawn tree,
    // only line up in a font that has them all: `box` gets the stylesheet to
    // give the block one.
    let mut classes = Vec::new();
    if !lang.is_empty() {
        classes.push(format!("language-{}", escape(lang)));
    }
    if code.chars().any(|c| ('\u{2500}'..='\u{257F}').contains(&c)) {
        classes.push("box".to_string());
    }
    let class = if classes.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", classes.join(" "))
    };
    format!("\n<pre><code{class}>{inner}\n</code></pre>\n")
}

/// A compiler report shown in a page opens on its code, `[E1412] Error: …`:
/// the code, brackets included, links to its entry. The highlighted HTML
/// has no `[` of its own, so every one is the text's.
fn report_codes(html: &str, highlighter: &StHighlighter) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let code = rest.get(1..6).filter(|_| rest.get(6..7) == Some("]"));
        match code.and_then(|c| highlighter.code_href(c)) {
            Some(href) => {
                out.push_str(&format!(
                    "<a class=\"code-link\" href=\"{href}\">{}</a>",
                    &rest[..7]
                ));
                rest = &rest[7..];
            }
            None => {
                out.push('[');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One of GitHub's alerts, `> [!NOTE]` through `> [!CAUTION]`, which GitHub
/// renders in the README and docs/. Zola does not know the syntax and would
/// print the marker inside a quote, so the block is rewritten in place: the
/// marker line becomes the opening tag and the title, the lines after it lose
/// their `>` and stay Markdown for Zola, which is what keeps the inline code
/// inside highlighted like any other, and a closing tag follows the quote.
/// Blank lines around the tags make them HTML blocks of their own.
fn alert(
    replacements: &mut Vec<(Range<usize>, String)>,
    body: &str,
    range: Range<usize>,
    kind: BlockQuoteKind,
) {
    // The same drawings GitHub uses, in the site's line weight.
    let (name, icon) = match kind {
        BlockQuoteKind::Note => (
            "note",
            r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5"/><path d="M12 8h.01"/>"#,
        ),
        BlockQuoteKind::Tip => (
            "tip",
            r#"<path d="M9.5 18h5"/><path d="M10.5 21h3"/><path d="M12 3a6 6 0 0 0-3.6 10.8c.7.6 1.1 1.3 1.1 2.2h5c0-.9.4-1.6 1.1-2.2A6 6 0 0 0 12 3z"/>"#,
        ),
        BlockQuoteKind::Important => (
            "important",
            r#"<path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v9a1.5 1.5 0 0 1-1.5 1.5H13l-4 4v-4H5.5A1.5 1.5 0 0 1 4 14.5z"/><path d="M12 7.5v4"/><path d="M12 14h.01"/>"#,
        ),
        BlockQuoteKind::Warning => (
            "warning",
            r#"<path d="M12 4 21 19.5H3z"/><path d="M12 10v4"/><path d="M12 17h.01"/>"#,
        ),
        BlockQuoteKind::Caution => (
            "caution",
            r#"<path d="M8.5 3h7l5 5v7l-5 5h-7l-5-5V8z"/><path d="M12 8v4"/><path d="M12 16h.01"/>"#,
        ),
    };
    let quote = &body[range.clone()];
    let first = quote.find('\n').map_or(quote.len(), |i| i + 1);
    replacements.push((
        range.start..range.start + first,
        format!(
            "<div class=\"alert alert-{name}\">\n<p class=\"alert-title\">{}{name}</p>\n\n",
            crate::site::icon(icon)
        ),
    ));
    let mut pos = first;
    while pos < quote.len() {
        let end = quote[pos..].find('\n').map_or(quote.len(), |i| pos + i + 1);
        let line = &quote[pos..end];
        let indent = line.len() - line.trim_start_matches(' ').len();
        if line[indent..].starts_with('>') {
            let mut marker = indent + 1;
            if line[marker..].starts_with(' ') {
                marker += 1;
            }
            replacements.push((range.start + pos..range.start + pos + marker, String::new()));
        }
        pos = end;
    }
    replacements.push((range.end..range.end, "\n</div>\n".to_string()));
}

/// The rough token count agents budget by: about four characters per token
/// for English and code alike. Reported, never relied on.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_becomes_a_titled_block_and_keeps_its_markdown() {
        let highlighter = StHighlighter::new();
        let body = "\
Before.

> [!TIP]
> Prefer `rk check` first.
>
> - a **list** item
>
> ```sh
> rk fmt
> ```
> last line

> a plain quote
";
        let out = preprocess(body, &highlighter).body;
        // The opening tag and the title replace the marker line; the quote
        // marks are gone, the inline code is highlighted, the fence is
        // rendered, and the closing tag follows the last line.
        assert!(
            out.starts_with(
                "Before.\n\n<div class=\"alert alert-tip\">\n<p class=\"alert-title\"><svg"
            ),
            "{out}"
        );
        assert!(out.contains("</svg>tip</p>\n\nPrefer <code>"), "{out}");
        assert!(out.contains("\n\n- a **list** item\n\n"), "{out}");
        assert!(out.contains("<pre><code class=\"language-sh\">"), "{out}");
        assert!(
            out.contains("</code></pre>\n\nlast line\n\n</div>\n\n> a plain quote\n"),
            "{out}"
        );
        let (alert, _) = out.split_once("</div>").unwrap();
        assert!(
            !alert.contains("[!TIP]") && !alert.contains("\n> "),
            "{out}"
        );
    }

    #[test]
    fn inline_code_of_several_tokens_is_colored_token_by_token() {
        let highlighter = StHighlighter::new();
        let html = |src: &str| highlighter.inline(src);
        assert_eq!(html(":="), "<span class=\"hl-operator\">:=</span>");
        assert_eq!(
            html("REF_TO REAL"),
            "<span class=\"hl-keyword-storage\">REF_TO</span> <span class=\"hl-type-builtin\">REAL</span>"
        );
        assert_eq!(
            html("x : INT := 5"),
            "x <span class=\"hl-operator\">:</span> <span class=\"hl-type-builtin\">INT</span> <span class=\"hl-operator\">:=</span> <span class=\"hl-number\">5</span>"
        );
        // Not code: no word of the table, no address, no `:=`.
        assert_eq!(html("rk_build/debug/core.wasm"), "rk_build/debug/core.wasm");
        assert_eq!(html("env.memory"), "env.memory");
    }

    #[test]
    fn a_heading_that_is_a_keyword_takes_its_class() {
        let highlighter = StHighlighter::new();
        let out = preprocess(
            "# Page\n\n## VAR_CONFIG\n\nText.\n\n## Split a configuration\n",
            &highlighter,
        )
        .body;
        assert!(out.contains("## VAR_CONFIG {.k-var-config}\n"), "{out}");
        assert!(out.contains("## Split a configuration\n"), "{out}");
    }

    #[test]
    fn a_fence_drawn_in_box_characters_is_marked() {
        let highlighter = StHighlighter::new();
        let body = "```sh\n[E1412] Error\n   ╭─[ main.st:7:17 ]\n```\n\n```sh\nrk check\n```\n";
        let out = preprocess(body, &highlighter).body;
        let (diagnostic, command) = out.split_once("</pre>").unwrap();
        assert!(
            diagnostic.contains("<code class=\"language-sh box\">"),
            "{out}"
        );
        assert!(command.contains("<code class=\"language-sh\">"), "{out}");
    }

    #[test]
    fn a_marker_with_text_after_it_stays_a_quote() {
        let highlighter = StHighlighter::new();
        let body = "> [!NOTE] not alone on its line\n> so GitHub shows it as a quote too\n";
        assert_eq!(preprocess(body, &highlighter).body, body);
    }
}
