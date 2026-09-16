//! The hand-written pages in `site/pages/`, mirrored into `site/content/`
//! with their fences and inline code pre-rendered. Zola's `+++` frontmatter
//! passes through untouched; the generator reads only the title, the lede
//! and the Markdown twin's path from it.

use std::path::{Path, PathBuf};

use crate::highlight::StHighlighter;
use crate::markdown::{self, Fence};

pub struct Page {
    /// Path under `site/pages/`, e.g. `formatter.md` or `skills/_index.md`.
    pub rel: PathBuf,
    /// The `+++` frontmatter, verbatim.
    pub frontmatter: String,
    /// The Markdown body as written: the twin's source.
    pub source: String,
    /// The body pre-rendered for Zola.
    pub body: String,
    pub fences: Vec<Fence>,
    pub title: String,
    pub lede: String,
    /// `extra.md`, the twin's site path, when the page declares one.
    pub md: Option<String>,
}

impl Page {
    /// The twin's path under `static/`: `index.md` beside the page's
    /// `index.html`.
    pub fn twin_rel(&self) -> PathBuf {
        let parent = self.rel.parent().unwrap_or(Path::new(""));
        let stem = self.rel.file_stem().unwrap().to_string_lossy();
        if stem == "_index" {
            parent.join("index.md")
        } else {
            parent.join(stem.as_ref()).join("index.md")
        }
    }
}

pub fn discover(dir: &Path, highlighter: &StHighlighter) -> Vec<Page> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let (frontmatter, source) = markdown::split_toml_frontmatter(&text);
            let pre = markdown::preprocess(source, highlighter);
            let fm_lines = text[..text.len() - source.len()].matches('\n').count();
            let mut fences = pre.fences;
            for f in &mut fences {
                f.line += fm_lines;
            }
            Page {
                rel: path.strip_prefix(dir).unwrap().to_path_buf(),
                title: toml_field(frontmatter, "title")
                    .unwrap_or_else(|| panic!("{}: no `title` in the frontmatter", path.display())),
                lede: toml_field(frontmatter, "lede").unwrap_or_default(),
                md: toml_field(frontmatter, "md"),
                frontmatter: frontmatter.to_string(),
                source: source.to_string(),
                body: pre.body,
                fences,
            }
        })
        .collect()
}

/// A `key = "value"` line of the frontmatter, unescaped. Enough for the
/// three fields the generator reads; Zola parses the rest.
fn toml_field(frontmatter: &str, key: &str) -> Option<String> {
    frontmatter.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            return None;
        }
        let v = v.trim();
        let v = v.strip_prefix('"')?.strip_suffix('"')?;
        Some(v.replace("\\\"", "\"").replace("\\\\", "\\"))
    })
}
