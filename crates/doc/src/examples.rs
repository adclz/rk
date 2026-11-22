// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The diagnostics reference's sources: one Markdown file per code in
//! `crates/doc/examples/`, named `<CODE>.md`. Its frontmatter carries the
//! title (and the lint rule, for an L-code); the body is the description
//! followed by the `iecst` fences that must produce the code, and a `toml`
//! fence when they need a setting: the `config.toml` they are compiled
//! with. A code with no fence is described without a runnable example.

use std::path::Path;

use crate::markdown;

/// The reference's sections in reading order. A code's first two digits name
/// its section, so a new code is filed by its number and nothing else.
pub const SECTIONS: &[(&str, &str)] = &[
    ("E00", "Syntax"),
    ("E01", "Duplicates"),
    ("E02", "Resolution"),
    ("E03", "Type System"),
    ("E04", "Initializers"),
    ("E05", "Arrays"),
    ("E06", "Enums"),
    ("E07", "Subranges"),
    ("E08", "Calls"),
    ("E09", "References"),
    ("E10", "Visibility"),
    ("E11", "OOP"),
    ("E12", "Control Flow"),
    ("E13", "Type cycles"),
    ("E14", "Configuration"),
    ("E15", "Pragmas"),
    ("L00", "Lint pragmas"),
    ("L01", "Linter Warning"),
    ("L02", "Linter Info"),
    ("L03", "Linter Hint"),
];

pub fn section_of(code: &str) -> &'static str {
    SECTIONS
        .iter()
        .find(|(prefix, _)| code.starts_with(prefix))
        .map(|(_, name)| *name)
        .unwrap_or_else(|| panic!("{code}: no section owns this code's range; add it to SECTIONS"))
}

pub struct ErrorExample {
    pub code: String,
    pub category: &'static str,
    pub title: String,
    pub description: String,
    /// The sources that must trigger the code. Empty for a code described
    /// without a runnable example.
    pub sources: Vec<String>,
    /// The `config.toml` the sources are compiled with, when they need a
    /// setting (`[addresses]`, `stack_size`): the example's `toml` fence.
    pub config: Option<String>,
    /// When set, only this lint rule is enabled for the example.
    pub lint_rule: Option<String>,
}

/// The codes with an entry on the diagnostics page: one file each in `dir`.
/// The pages link a code they name to its entry, and cannot name another.
pub fn codes(dir: &Path) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect()
}

/// Every example in `dir`, sorted by code. A malformed file is a panic
/// naming it: the reference cannot be published with a hole.
pub fn load(dir: &Path) -> Vec<ErrorExample> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let code = path.file_stem().unwrap().to_string_lossy().to_string();
            let text = std::fs::read_to_string(&path).unwrap();
            let (fm, body) = markdown::split_frontmatter(&text);
            let title = fm
                .get("title")
                .unwrap_or_else(|| panic!("{}: no `title` in the frontmatter", path.display()))
                .to_string();
            let lint_rule = fm.get("lint_rule").map(str::to_string);
            let (description, sources, config) = split_body(body);
            if description.is_empty() {
                panic!("{}: no description before the first fence", path.display());
            }
            if config.is_some() && sources.is_empty() {
                panic!(
                    "{}: a `toml` fence with no source to compile",
                    path.display()
                );
            }
            ErrorExample {
                category: section_of(&code),
                code,
                title,
                description,
                sources,
                config,
                lint_rule,
            }
        })
        .collect()
}

/// The prose before the first fence, every `iecst` fence, and the one `toml`
/// fence. A description is collapsed within each paragraph, so a source line
/// break is free, and its blank lines survive as the paragraphs the page and
/// `rk explain` print.
fn split_body(body: &str) -> (String, Vec<String>, Option<String>) {
    let mut description = String::new();
    let mut sources = Vec::new();
    let mut config: Option<String> = None;
    // The fence open, and whether it is the `toml` one.
    let mut fence: Option<(String, bool)> = None;
    let mut seen_fence = false;
    for line in body.lines() {
        if let Some((code, _)) = fence.as_mut() {
            if line.starts_with("```") {
                match fence.take().unwrap() {
                    (code, true) => config = Some(code.trim_matches('\n').to_string()),
                    (code, false) => sources.push(code),
                }
            } else {
                code.push_str(line);
                code.push('\n');
            }
            continue;
        }
        if let Some(info) = line.strip_prefix("```") {
            let toml = match info.trim() {
                "iecst" => false,
                "toml" => {
                    assert!(config.is_none(), "an example has one `toml` fence");
                    true
                }
                _ => panic!(
                    "an example's fences are `iecst`, and one `toml` for its config.toml, found `{line}`"
                ),
            };
            fence = Some((String::new(), toml));
            seen_fence = true;
            continue;
        }
        if !seen_fence {
            description.push_str(line);
            description.push('\n');
        }
    }
    assert!(fence.is_none(), "unterminated fence");
    let description = description
        .split("\n\n")
        .map(|para| para.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|para| !para.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let sources = sources
        .into_iter()
        .map(|s| s.trim_matches('\n').to_string())
        .collect();
    (description, sources, config)
}
