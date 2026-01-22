//! Corpus loading and management for IEC ST fuzzing.
//!
//! The corpus can be generated from tree-sitter test files by extracting
//! the code sections (everything before the "----" separator).

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

/// Create a seed corpus from tree-sitter test files, one entry per
/// extracted source.
pub fn create_seed_corpus(source_dir: &Path, output_dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(output_dir)?;

    let mut count = 0;

    // Process all .txt files in the source directory (non-recursive)
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && path.extension().is_some_and(|ext| ext == "txt")
            && let Ok(contents) = std::fs::read_to_string(&path)
        {
            let extracted_list = extract_all_sources_from_corpus(&contents);
            for extracted in extracted_list {
                if !extracted.is_empty() {
                    save_corpus_entry(&extracted, output_dir, &mut count)?;
                }
            }
        }
    }

    Ok(count)
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
    use std::io::Write;

    let filename = format!("seed-{:06}.st", *count);
    let path = output_dir.join(filename);

    let mut file = std::fs::File::create(&path)?;
    file.write_all(source.as_bytes())?;

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
}
