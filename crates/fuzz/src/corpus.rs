// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The seed corpus: every piece of Structured Text the repository already
//! holds, one file per program.
//!
//! libFuzzer mutates what it is given, and random bytes almost never make a
//! program the compiler accepts, so the codegen oracles are only reached
//! from seeds that are close to one. Hence all four kinds of source:
//!
//! - `.txt` — tree-sitter corpus tests: the text before the `---` separator.
//!   Every syntactic form, mostly not well-typed.
//! - `.st` — the standard library and any workspace: accepted code.
//! - `.md` — the `iecst`/`st` fences of the docs, skills and diagnostic
//!   examples: one accepted program or one error each.
//! - `.rs` — raw strings holding ST in the test suite (`src/tests/`): the
//!   codegen tests' programs are accepted AND executed, the closest seeds
//!   to every runtime oracle there is.

use std::path::Path;

/// Load all files from a corpus directory.
pub fn load_corpus(dir: &Path) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut corpus = Vec::new();

    if !dir.exists() {
        anyhow::bail!("corpus directory does not exist: {}", dir.display());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            match std::fs::read(&path) {
                Ok(data) => corpus.push(data),
                Err(e) => {
                    eprintln!("Warning: failed to read {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(corpus)
}

/// Extract every program under `source` (a file or a directory, walked
/// recursively) into `output_dir`, one file each; returns how many were
/// written. Names are numbered from `start`, so several sources can fill
/// one directory.
pub fn create_seed_corpus(source: &Path, output_dir: &Path, start: usize) -> anyhow::Result<usize> {
    std::fs::create_dir_all(output_dir)?;
    let output_dir = output_dir.canonicalize()?;
    let mut count = start;
    walk(source, &output_dir, &mut count)?;
    Ok(count - start)
}

fn walk(path: &Path, output_dir: &Path, count: &mut usize) -> anyhow::Result<()> {
    if path.is_dir() {
        // Build outputs are not sources, and an output directory inside the
        // tree would feed on itself.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(name, "target" | "node_modules" | "rk_build" | ".git")
            || path.canonicalize().is_ok_and(|p| p == output_dir)
        {
            return Ok(());
        }
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for entry in entries {
            walk(&entry, output_dir, count)?;
        }
        return Ok(());
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    let sources = match path.extension().and_then(|e| e.to_str()) {
        Some("txt") => extract_all_sources_from_corpus(&contents),
        Some("st") => vec![contents],
        Some("md") => extract_st_fences(&contents),
        Some("rs") => extract_st_raw_strings(&contents),
        _ => Vec::new(),
    };
    for source in sources {
        if !source.trim().is_empty() {
            save_corpus_entry(&source, output_dir, count)?;
        }
    }
    Ok(())
}

/// The bodies of the ```` ```iecst ```` and ```` ```st ```` fences, whatever
/// follows the tag (`fragment`, `continues`): a fragment is still text the
/// parser must survive.
fn extract_st_fences(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in content.lines() {
        let trimmed = line.trim_start();
        match &mut current {
            None => {
                if let Some(tag) = trimmed.strip_prefix("```") {
                    let lang = tag.split_whitespace().next().unwrap_or("");
                    if matches!(lang, "iecst" | "st") {
                        current = Some(Vec::new());
                    }
                }
            }
            Some(lines) => {
                if trimmed.starts_with("```") {
                    out.push(lines.join("\n"));
                    current = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    out
}

/// Rust raw strings (`r"…"`, `r#"…"#`, …) that hold a POU: the test
/// suite's programs, with their indentation as written.
fn extract_st_raw_strings(content: &str) -> Vec<String> {
    const MARKERS: [&str; 6] = [
        "END_FUNCTION",
        "END_PROGRAM",
        "END_TYPE",
        "END_CLASS",
        "END_INTERFACE",
        "END_CONFIGURATION",
    ];
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        // `r` opening a raw string, not the end of an identifier.
        let starts = bytes[i] == b'r'
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'));
        if !starts {
            i += 1;
            continue;
        }
        let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
        let open = i + 1 + hashes;
        if bytes.get(open) != Some(&b'"') {
            i += 1;
            continue;
        }
        let close: String = std::iter::once('"')
            .chain(std::iter::repeat_n('#', hashes))
            .collect();
        let body_start = open + 1;
        let Some(len) = content[body_start..].find(&close) else {
            break;
        };
        let body = &content[body_start..body_start + len];
        if MARKERS.iter().any(|m| body.contains(m)) {
            out.push(body.to_string());
        }
        i = body_start + len + close.len();
    }
    out
}

/// Extract every source code section from a tree-sitter corpus file (the
/// text between a title banner and the `----` separator).
fn extract_all_sources_from_corpus(content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Look for the start of a test case (line of = signs)
        if lines[i].starts_with("=") && lines[i].len() >= 10 {
            i += 1;

            // Skip the title line
            if i < lines.len() && !lines[i].starts_with("=") {
                i += 1;
            }

            // Skip the closing = line
            if i < lines.len() && lines[i].starts_with("=") {
                i += 1;
            }

            // Skip any empty lines
            while i < lines.len() && lines[i].is_empty() {
                i += 1;
            }

            // Collect source lines until we hit the separator
            let mut source_lines = Vec::new();
            while i < lines.len() {
                let line = lines[i];
                // Stop at the separator line (all dashes)
                if line.trim().starts_with("---") {
                    break;
                }
                source_lines.push(line);
                i += 1;
            }

            // Remove trailing empty lines
            while source_lines.last().is_some_and(|l| l.is_empty()) {
                source_lines.pop();
            }

            let source = source_lines.join("\n");
            if !source.trim().is_empty() {
                results.push(source);
            }
        } else {
            i += 1;
        }
    }

    results
}

fn save_corpus_entry(source: &str, output_dir: &Path, count: &mut usize) -> anyhow::Result<()> {
    let path = output_dir.join(format!("seed-{:06}.st", *count));
    std::fs::write(&path, source.as_bytes())?;
    *count += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_source_simple() {
        let corpus = r#"================================================================================
Simple Function
================================================================================

FUNCTION test
END_FUNCTION

----------------

(source_file
  (source_element
    (function_decl ...)))"#;

        let sources = extract_all_sources_from_corpus(corpus);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].trim(), "FUNCTION test\nEND_FUNCTION");
    }

    #[test]
    fn test_extract_source_multiline() {
        let corpus = r#"================================================================================
Complex Example
================================================================================

FUNCTION_BLOCK MyBlock
    VAR
        x : INT;
    END_VAR
END_FUNCTION_BLOCK

----------------

(source_file ...)"#;

        let sources = extract_all_sources_from_corpus(corpus);
        assert_eq!(sources.len(), 1);
        assert!(sources[0].contains("FUNCTION_BLOCK MyBlock"));
        assert!(sources[0].contains("VAR"));
        assert!(!sources[0].contains("----"));
    }

    #[test]
    fn test_extract_source_empty() {
        let corpus = r#"================================================================================
Empty
================================================================================

----------------

(source_file)"#;

        let sources = extract_all_sources_from_corpus(corpus);
        assert!(sources.is_empty());
    }

    #[test]
    fn test_extract_multiple_sources() {
        let corpus = r#"================================================================================
First Test
================================================================================

FUNCTION first
END_FUNCTION

----------------

(source_file)

================================================================================
Second Test
================================================================================

FUNCTION second
END_FUNCTION

----------------

(source_file)"#;

        let sources = extract_all_sources_from_corpus(corpus);
        assert_eq!(sources.len(), 2);
        assert!(sources[0].contains("first"));
        assert!(sources[1].contains("second"));
    }

    #[test]
    fn fences_tagged_st_are_extracted_and_others_are_not() {
        let md = "text\n```iecst fragment\nx := 1;\n```\n```sh\nrk check\n```\n```st\nFUNCTION f\nEND_FUNCTION\n```\n";
        assert_eq!(
            extract_st_fences(md),
            vec![
                "x := 1;".to_string(),
                "FUNCTION f\nEND_FUNCTION".to_string()
            ]
        );
    }

    #[test]
    fn raw_strings_holding_a_pou_are_extracted() {
        let rs = concat!(
            "let a = r#\"FUNCTION f : INT\n  f := \"x\";\nEND_FUNCTION\"#;\n",
            "let b = r\"not st\";\n",
            "let c = br\"PROGRAM p END_PROGRAM\";\n",
            "let d = r\"PROGRAM p\nEND_PROGRAM\";\n",
        );
        let found = extract_st_raw_strings(rs);
        assert_eq!(
            found,
            vec![
                "FUNCTION f : INT\n  f := \"x\";\nEND_FUNCTION".to_string(),
                "PROGRAM p\nEND_PROGRAM".to_string(),
            ]
        );
    }
}
