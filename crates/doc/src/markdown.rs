//! Markdown as the site's sources write it: split the frontmatter, find the
//! fences the compiler must check, and pre-render what Zola cannot, which is
//! `iecst` fences through the grammar's highlighter and inline code with the
//! same classes. Everything else stays Markdown for Zola to render.

use std::collections::BTreeMap;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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

/// Pre-render a Markdown body for Zola.
pub fn preprocess(body: &str, highlighter: &StHighlighter) -> Preprocessed {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(body, options).into_offset_iter();

    let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
    let mut fences = Vec::new();
    let mut title: Option<String> = None;
    let mut in_fence: Option<(String, usize, Range<usize>, String)> = None;
    let mut h1: Option<(Range<usize>, String)> = None;

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
            Event::Code(text) => {
                replacements.push((range, format!("<code>{}</code>", highlighter.inline(&text))));
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
    let inner = match lang {
        // `pascal` is what the README marks Structured Text as, because that is
        // what GitHub highlights it as; here it is the same grammar.
        "iecst" | "pascal" => {
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
    let class = if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{}\"", escape(lang))
    };
    format!("\n<pre><code{class}>{inner}\n</code></pre>\n")
}

/// The rough token count agents budget by: about four characters per token
/// for English and code alike. Reported, never relied on.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}
