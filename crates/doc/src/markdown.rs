//! Markdown as the site's source format: frontmatter, the body rendered to
//! HTML, and every fenced `iecst` block collected so the compiler can vet it.

use std::collections::BTreeMap;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::highlight::{StHighlighter, escape};

/// The YAML frontmatter of a skill or reference file, reduced to what the
/// site needs. Values are single strings; a folded multi-line value is
/// joined with spaces, which is what YAML does for a plain scalar.
#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    pub fields: BTreeMap<String, String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Split `---` frontmatter from the body. A file without frontmatter is all
/// body.
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

/// One fenced code block, with where it sits so a failing example can be
/// named by file and line.
#[derive(Debug, Clone)]
pub struct Fence {
    /// The info string after the backticks, e.g. `iecst expect=E0101`.
    pub info: String,
    pub code: String,
    /// 1-based line of the opening fence.
    pub line: usize,
}

pub struct Rendered {
    pub html: String,
    /// The first H1's text, if the body has one.
    pub title: Option<String>,
    pub fences: Vec<Fence>,
}

/// Render a Markdown body to HTML. `iecst` fences are highlighted by the
/// grammar; other fences keep their language as a class and no styling.
pub fn render(body: &str, highlighter: &StHighlighter) -> Rendered {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(body, options).into_offset_iter();

    let mut html = String::new();
    let mut title = None;
    let mut fences = Vec::new();
    let mut events: Vec<Event> = Vec::new();

    // State for the block being collected.
    let mut in_fence: Option<(String, usize, String)> = None;
    let mut in_h1 = false;
    let mut h1_text = String::new();

    for (event, range) in parser {
        match &event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let line = body[..range.start].matches('\n').count() + 1;
                in_fence = Some((info.to_string(), line, String::new()));
                continue;
            }
            Event::Text(text) if in_fence.is_some() => {
                in_fence.as_mut().unwrap().2.push_str(text);
                continue;
            }
            Event::End(TagEnd::CodeBlock) => {
                let (info, line, code) = in_fence.take().unwrap();
                let lang = info.split_whitespace().next().unwrap_or("");
                let inner = if lang == "iecst" {
                    highlighter.html(code.trim_end_matches('\n'))
                } else {
                    escape(code.trim_end_matches('\n'))
                };
                let class = if lang.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"language-{}\"", escape(lang))
                };
                events.push(Event::Html(CowStr::from(format!(
                    "<pre><code{class}>{inner}\n</code></pre>\n"
                ))));
                if lang == "iecst" {
                    fences.push(Fence { info, code, line });
                }
                continue;
            }
            Event::Start(Tag::Heading { level, .. })
                if *level == pulldown_cmark::HeadingLevel::H1 && title.is_none() =>
            {
                in_h1 = true;
            }
            Event::Text(text) if in_h1 => h1_text.push_str(text),
            Event::Code(text) if in_h1 => h1_text.push_str(text),
            Event::End(TagEnd::Heading(pulldown_cmark::HeadingLevel::H1)) if in_h1 => {
                in_h1 = false;
                title = Some(std::mem::take(&mut h1_text));
            }
            _ => {}
        }
        events.push(event);
    }

    pulldown_cmark::html::push_html(&mut html, events.into_iter());
    Rendered {
        html,
        title,
        fences,
    }
}

/// The rough token count agents budget by: about four characters per token
/// for English and code alike. Reported, never relied on.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}
