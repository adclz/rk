//! The documentation in `docs/`: plain Markdown, one file per page, so GitHub
//! renders it as it is. The site takes each file's H1 as the title and its
//! first paragraph as the description, and builds its sidebar from the list
//! in `docs/README.md`, so the list GitHub shows for the folder and the one
//! the site shows are the same list.

use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde_json::{Value, json};

use crate::highlight::StHighlighter;
use crate::markdown::{self, Fence};
use crate::site::one_line;

pub struct DocPage {
    /// The file name without `.md`: `profiles`, served at `/docs/profiles/`.
    pub stem: String,
    pub title: String,
    pub description: String,
    /// As written, with its links pointed at the site: the Markdown twin.
    pub source: String,
    /// Pre-rendered for Zola, without the H1 the template prints.
    pub body: String,
    pub fences: Vec<Fence>,
}

impl DocPage {
    pub fn url(&self) -> String {
        format!("/docs/{}/", self.stem)
    }
}

pub fn discover(repo: &Path, highlighter: &StHighlighter) -> Vec<DocPage> {
    let dir = repo.join("docs");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        // The list of pages, which is the sidebar and no page of its own.
        .filter(|path| path.file_name().is_some_and(|n| n != INDEX))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            // Rewriting a link never adds or removes a line, so the fences
            // keep the line numbers of the file as written.
            let source = links_for_site(&text, "docs");
            let pre = markdown::preprocess(&source, highlighter);
            DocPage {
                stem: path.file_stem().unwrap().to_string_lossy().into_owned(),
                title: pre
                    .title
                    .unwrap_or_else(|| panic!("{}: the page has no `# Title`", path.display())),
                description: first_paragraph(&text),
                source,
                body: pre.body,
                fences: pre.fences,
            }
        })
        .collect()
}

/// The first paragraph or list item after the H1, as one line of plain text:
/// what search results and `llms.txt` show for the page. A table, a quote or
/// a fence says too little on its own, so the first prose after them wins,
/// less the colon that introduced whatever follows it.
fn first_paragraph(text: &str) -> String {
    let mut out = String::new();
    let mut skip = 0usize;
    let mut inside = false;
    for event in Parser::new_ext(text, markdown::options()) {
        match event {
            Event::Start(
                Tag::Heading { .. } | Tag::Table(_) | Tag::BlockQuote(_) | Tag::CodeBlock(_),
            ) => skip += 1,
            Event::End(
                TagEnd::Heading(_) | TagEnd::Table | TagEnd::BlockQuote(_) | TagEnd::CodeBlock,
            ) => skip -= 1,
            Event::Start(Tag::Paragraph | Tag::Item) if skip == 0 => inside = true,
            Event::End(TagEnd::Paragraph | TagEnd::Item) if inside => break,
            Event::Text(t) | Event::Code(t) if inside => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak if inside => out.push(' '),
            _ => {}
        }
    }
    one_line(&out).trim_end_matches(':').to_string()
}

/// Where a link written for GitHub points on the site. `from` is the
/// directory of the file it is written in, relative to the repository. A page
/// in `docs/` is served at `/docs/<stem>/`, a skill at `/skills/<name>/` and
/// the README is the front page; any other path in the repository only
/// resolves on GitHub. Absolute, rooted and fragment links are left alone.
pub fn links_for_site(text: &str, from: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        out.push_str(&rest[..i + 2]);
        rest = &rest[i + 2..];
        let end = rest.find(')').unwrap_or(rest.len());
        out.push_str(&site_target(&rest[..end], from));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn site_target(target: &str, from: &str) -> String {
    const BLOB: &str = "https://github.com/adclz/rk/blob/main/";
    if target.starts_with("http")
        || target.starts_with("mailto:")
        || target.starts_with('/')
        || target.starts_with('#')
    {
        return target.to_string();
    }
    let (path, fragment) = target.split_at(target.find('#').unwrap_or(target.len()));
    let path = resolve(from, path);
    if path == "README.md" {
        return format!("/{fragment}");
    }
    if let Some(stem) = path
        .strip_prefix("docs/")
        .and_then(|p| p.strip_suffix(".md"))
        && !stem.contains('/')
    {
        return format!("/docs/{stem}/{fragment}");
    }
    if let Some(page) = path
        .strip_prefix("skills/")
        .and_then(|p| p.strip_suffix(".md"))
    {
        // `name/SKILL` is the skill's own page, `name/references/x` a page of
        // its own under it.
        let page = page.strip_suffix("/SKILL").unwrap_or(page);
        return format!("/skills/{page}/{fragment}");
    }
    format!("{BLOB}{path}{fragment}")
}

/// `path` as written in a file under `from`, relative to the repository.
fn resolve(from: &str, path: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').filter(|p| !p.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let mut out = parts.join("/");
    // A directory keeps its slash, which is how GitHub knows to list it.
    if path.ends_with('/') && !out.is_empty() {
        out.push('/');
    }
    out
}

/// One link in the sidebar.
#[derive(Clone)]
pub struct Entry {
    pub title: String,
    pub url: String,
}

pub enum Item {
    Page(Entry),
    Group(String, Vec<Entry>),
}

pub struct Sidebar {
    pub items: Vec<Item>,
}

/// The file in `docs/` that lists the pages.
pub const INDEX: &str = "README.md";

impl Sidebar {
    /// The list in `docs/README.md`, read as the site sees it: a link to a
    /// page of `docs/` is a page, an item without a link opens a group, and a
    /// link to the README is the front page, which is no page of the docs.
    /// The `tools` close it.
    ///
    /// Refuses a list that names a page `docs/` does not have, or leaves one
    /// out, so no page is unreachable from the sidebar.
    pub fn from_index(
        index: &str,
        docs: &[DocPage],
        tools: Vec<Entry>,
    ) -> Result<Self, Vec<String>> {
        let mut problems = Vec::new();
        let mut items = Vec::new();
        let mut listed = Vec::new();
        let list = index.lines().filter(|l| l.trim_start().starts_with("- "));
        for line in list {
            let nested = line.starts_with([' ', '\t']);
            let text = line.trim_start()[2..].trim();
            let Some((title, target)) = link(text) else {
                if nested {
                    problems.push(format!("docs/{INDEX}: `{text}` in the list links nowhere"));
                } else {
                    items.push(Item::Group(text.to_string(), Vec::new()));
                }
                continue;
            };
            if target.starts_with("../README.md") && !nested {
                continue; // the front page
            }
            let Some(page) = target
                .strip_suffix(".md")
                .filter(|stem| !stem.contains('/'))
                .and_then(|stem| docs.iter().find(|d| d.stem == stem))
            else {
                problems.push(format!(
                    "docs/{INDEX}: the list links `{title}` to `{target}`, which is not a page in docs/"
                ));
                continue;
            };
            listed.push(page.stem.as_str());
            let entry = Entry {
                title: title.to_string(),
                url: page.url(),
            };
            match items.last_mut() {
                Some(Item::Group(_, children)) if nested => children.push(entry),
                _ if nested => problems.push(format!(
                    "docs/{INDEX}: `{title}` is nested under an item that is not a group"
                )),
                _ => items.push(Item::Page(entry)),
            }
        }
        for d in docs {
            if !listed.contains(&d.stem.as_str()) {
                problems.push(format!(
                    "docs/{}.md is not in the list in docs/{INDEX}, so nothing links to it",
                    d.stem
                ));
            }
        }
        if !tools.is_empty() {
            items.push(Item::Group("Tools".into(), tools));
        }
        if problems.is_empty() {
            Ok(Self { items })
        } else {
            Err(problems)
        }
    }

    /// Every page in reading order: what previous and next follow.
    pub fn pages(&self) -> Vec<&Entry> {
        self.items
            .iter()
            .flat_map(|item| match item {
                Item::Page(e) => std::slice::from_ref(e).iter(),
                Item::Group(_, children) => children.iter(),
            })
            .collect()
    }

    /// What the templates read: the tree, the paths that show it, and each
    /// page's neighbours.
    pub fn json(&self) -> Value {
        let entry = |e: &Entry| json!({ "title": e.title, "url": e.url });
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|item| match item {
                Item::Page(e) => entry(e),
                Item::Group(title, children) => json!({
                    "title": title,
                    "children": children.iter().map(entry).collect::<Vec<_>>(),
                }),
            })
            .collect();
        let pages = self.pages();
        let mut pager = serde_json::Map::new();
        for (i, e) in pages.iter().enumerate() {
            let neighbour = |j: Option<usize>| {
                j.and_then(|j| pages.get(j))
                    .map_or(Value::Null, |e| entry(e))
            };
            pager.insert(
                e.url.clone(),
                json!({ "prev": neighbour(i.checked_sub(1)), "next": neighbour(Some(i + 1)) }),
            );
        }
        json!({
            "items": items,
            "paths": pages.iter().map(|e| e.url.as_str()).collect::<Vec<_>>(),
            "pager": pager,
        })
    }
}

/// `[title](target)`, the whole of a list item.
fn link(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('[')?;
    let (title, rest) = rest.split_once("](")?;
    let target = rest.strip_suffix(')')?;
    Some((title, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(stem: &str) -> DocPage {
        DocPage {
            stem: stem.into(),
            title: stem.into(),
            description: String::new(),
            source: String::new(),
            body: String::new(),
            fences: Vec::new(),
        }
    }

    #[test]
    fn links_point_at_the_site_or_at_github() {
        let text = "[a](strings.md) [b](strings.md#slots) [c](../README.md#license) \
                    [d](../stdlib/Convert.st) [e](#local) [f](https://x.dev) [g](../stdlib/)";
        assert_eq!(
            links_for_site(text, "docs"),
            "[a](/docs/strings/) [b](/docs/strings/#slots) [c](/#license) \
             [d](https://github.com/adclz/rk/blob/main/stdlib/Convert.st) [e](#local) \
             [f](https://x.dev) [g](https://github.com/adclz/rk/blob/main/stdlib/)"
        );
        // A skill and its references are pages of the site too.
        assert_eq!(
            links_for_site(
                "[s](../skills/programming-config/SKILL.md) [r](../skills/cli-compile/references/abi.md)",
                "docs"
            ),
            "[s](/skills/programming-config/) [r](/skills/cli-compile/references/abi/)"
        );
        // From the README, the same pages are one directory down.
        assert_eq!(
            links_for_site("[p](docs/profiles.md) [l](LICENSE)", ""),
            "[p](/docs/profiles/) [l](https://github.com/adclz/rk/blob/main/LICENSE)"
        );
    }

    #[test]
    fn the_first_prose_is_the_description() {
        let text = "# Env\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> [!NOTE]\n> a note\n\nThe `.env` file\nis read **first**.\n\nMore.\n";
        assert_eq!(first_paragraph(text), "The .env file is read first.");
        assert_eq!(
            first_paragraph("# A\n\nWrite it so:\n\n```sh\nx\n```\n"),
            "Write it so"
        );
        assert_eq!(
            first_paragraph("# Syntax\n\n- Semicolons are optional.\n- Case does not matter.\n"),
            "Semicolons are optional."
        );
    }

    #[test]
    fn the_docs_list_is_the_sidebar() {
        let index = "# Documentation\n\n- [Syntax](syntax.md)\n- Basics\n  - [Strings](strings.md)\n- [License](../README.md#license)\n";
        let docs = [page("syntax"), page("strings")];
        let tools = vec![Entry {
            title: "Linter".into(),
            url: "/linter/".into(),
        }];
        let sidebar = Sidebar::from_index(index, &docs, tools).unwrap_or_else(|p| panic!("{p:?}"));
        let urls: Vec<&str> = sidebar.pages().iter().map(|e| e.url.as_str()).collect();
        assert_eq!(urls, ["/docs/syntax/", "/docs/strings/", "/linter/"]);
        let json = sidebar.json();
        assert_eq!(json["items"][1]["title"], "Basics");
        assert_eq!(json["items"][1]["children"][0]["url"], "/docs/strings/");
        assert_eq!(json["pager"]["/docs/syntax/"]["prev"], Value::Null);
        assert_eq!(json["pager"]["/docs/strings/"]["prev"]["title"], "Syntax");
        assert_eq!(json["pager"]["/linter/"]["next"], Value::Null);
    }

    #[test]
    fn a_page_the_list_misses_or_invents_is_refused() {
        let index = "# Documentation\n\n- [Syntax](syntax.md)\n- [Gone](gone.md)\n";
        let problems = Sidebar::from_index(index, &[page("syntax"), page("orphan")], Vec::new())
            .err()
            .unwrap();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("`gone.md`"), "{problems:?}");
        assert!(problems[1].contains("docs/orphan.md"), "{problems:?}");
    }
}
