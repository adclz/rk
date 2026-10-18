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
//!
//! `fuzz_semantics` reads none of it. Its input is the bytes
//! [`crate::generate::program`] draws its choices from, where a program's
//! text is noise that still costs a compilation, a run and a formatting
//! each night. It starts from strings of bytes instead.

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
    let seeds = Seeds::of(&output_dir);
    if matches!(seeds, Seeds::Choices) {
        return Ok(0);
    }
    let mut count = start;
    walk(source, &output_dir, &mut count, seeds)?;
    Ok(count - start)
}

/// Remove the seeds an earlier run wrote into `output_dir`; returns how
/// many there were. The corpus is kept from night to night, and a seed of
/// a program the repository no longer holds would be replayed on each of
/// them. What libFuzzer kept is named by its hash, and stays.
pub fn remove_seeds(output_dir: &Path) -> anyhow::Result<usize> {
    if !output_dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in std::fs::read_dir(output_dir)? {
        let path = entry?.path();
        let is_seed = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(SEED_PREFIX));
        if is_seed && path.is_file() {
            std::fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Write the seeds of a target that reads no source; returns how many.
/// `None` for a target that reads programs, which has none of the kind.
pub fn create_choice_seeds(output_dir: &Path) -> anyhow::Result<Option<usize>> {
    std::fs::create_dir_all(output_dir)?;
    let output_dir = output_dir.canonicalize()?;
    if !matches!(Seeds::of(&output_dir), Seeds::Choices) {
        return Ok(None);
    }
    for seed in 0..CHOICE_SEEDS {
        let path = output_dir.join(format!("{SEED_PREFIX}{seed:06}.bin"));
        std::fs::write(&path, choices(seed as u64))?;
    }
    Ok(Some(CHOICE_SEEDS))
}

/// What every seed's file name starts with.
const SEED_PREFIX: &str = "seed-";

/// How many strings of choices `fuzz_semantics` starts from. Each is
/// replayed at the start of every night, a compilation and a run apiece.
const CHOICE_SEEDS: usize = 64;

/// The length of one. `generate::program` read 850 bytes for the median
/// program of forty random strings and 1,961 for the longest. A byte past
/// the end reads as 0, so a shorter string would end its program on the
/// first choice of every list.
const CHOICE_SEED_LEN: usize = 2048;

/// The string of choices number `seed`: SplitMix64, so the seeds are the
/// same on every machine and every night.
fn choices(seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = Vec::with_capacity(CHOICE_SEED_LEN);
    while bytes.len() < CHOICE_SEED_LEN {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        bytes.extend_from_slice(&(z ^ (z >> 31)).to_le_bytes());
    }
    bytes
}

/// What a corpus holds: the programs, or, for `fuzz_debug_format`, the
/// debug tables of those that compile, each as a module of its custom
/// sections, or, for `fuzz_semantics`, the choices its generator makes.
#[derive(Clone, Copy)]
enum Seeds {
    Sources,
    DebugModules,
    Choices,
}

impl Seeds {
    /// The target's own corpus directory says what it reads.
    fn of(output_dir: &Path) -> Self {
        match output_dir.file_name().and_then(|n| n.to_str()) {
            Some("fuzz_debug_format") => Seeds::DebugModules,
            Some("fuzz_semantics") => Seeds::Choices,
            _ => Seeds::Sources,
        }
    }
}

/// The custom sections of the debug build of `source`, as a module of
/// their own: what `fuzz_debug_format` reads. `None` when the source does
/// not compile on its own.
pub(crate) fn debug_module(source: &str) -> Option<Vec<u8>> {
    let (db, file) = crate::session::load(source)?;
    if !crate::session::compiles(&db, file) {
        return None;
    }
    let mir = crate::session::lower(&db, file).ok()?;
    // A program codegen refuses is `fuzz_compiler`'s finding, not a seed.
    let wasm = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wasm_codegen::generate_wasm_profile(&db, &mir, wasm_codegen::Profile::Debug).finish()
    }))
    .ok()?;
    Some(custom_sections_only(&wasm))
}

/// `wasm` without its code: the custom sections, in a module of their own.
fn custom_sections_only(wasm: &[u8]) -> Vec<u8> {
    fn leb128(mut v: u32, out: &mut Vec<u8>) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        let Ok(wasmparser::Payload::CustomSection(section)) = payload else {
            continue;
        };
        let mut body = Vec::new();
        leb128(section.name().len() as u32, &mut body);
        body.extend_from_slice(section.name().as_bytes());
        body.extend_from_slice(section.data());
        module.push(0);
        leb128(body.len() as u32, &mut module);
        module.extend_from_slice(&body);
    }
    module
}

fn walk(path: &Path, output_dir: &Path, count: &mut usize, seeds: Seeds) -> anyhow::Result<()> {
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
            walk(&entry, output_dir, count, seeds)?;
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
        if source.trim().is_empty() {
            continue;
        }
        match seeds {
            Seeds::Sources => save_corpus_entry(&source, output_dir, count)?,
            Seeds::DebugModules => {
                if let Some(module) = debug_module(&source) {
                    let path = output_dir.join(format!("{SEED_PREFIX}{:06}.wasm", *count));
                    std::fs::write(&path, &module)?;
                    *count += 1;
                }
            }
            // `create_seed_corpus` walks nothing for it.
            Seeds::Choices => {}
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
    let path = output_dir.join(format!("{SEED_PREFIX}{:06}.st", *count));
    std::fs::write(&path, source.as_bytes())?;
    *count += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty corpus directory of `target`, in a place of this test's own.
    fn corpus_of(target: &str, test: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("rk-fuzz-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join(target);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// `fuzz_semantics` was given the repository's programs for two weeks:
    /// 3,500 files its generator read as noise, replayed for 90 minutes of
    /// a 30 minute night, which then ran 57 inputs.
    #[test]
    fn the_semantics_target_starts_from_choices_and_reads_no_source() {
        let dir = corpus_of("fuzz_semantics", "choices");
        let source = dir.parent().unwrap().join("a.st");
        std::fs::write(&source, "FUNCTION f : INT\nEND_FUNCTION\n").unwrap();
        // What the kept corpus holds: a seed of an earlier night, and an
        // input libFuzzer found.
        std::fs::write(dir.join("seed-000007.st"), "PROGRAM p\nEND_PROGRAM\n").unwrap();
        std::fs::write(
            dir.join("0a4d55a8d778e5022fab701977c5d840bbc486d0"),
            [1, 2, 3],
        )
        .unwrap();

        assert_eq!(remove_seeds(&dir).unwrap(), 1);
        assert_eq!(create_choice_seeds(&dir).unwrap(), Some(CHOICE_SEEDS));
        assert_eq!(create_seed_corpus(&source, &dir, 0).unwrap(), 0);

        let names = names(&dir);
        assert_eq!(names.len(), CHOICE_SEEDS + 1, "{names:?}");
        assert_eq!(names[0], "0a4d55a8d778e5022fab701977c5d840bbc486d0");
        assert_eq!(names[1], "seed-000000.bin");
        assert_eq!(names[CHOICE_SEEDS], "seed-000063.bin");

        // Every seed is a program of its own, and more of one than the
        // program no choice at all makes.
        let least = crate::generate::program(&[]).lines().count();
        let mut programs = std::collections::BTreeSet::new();
        for name in &names[1..] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert_eq!(bytes.len(), CHOICE_SEED_LEN, "{name}");
            let program = crate::generate::program(&bytes);
            assert!(program.lines().count() > least, "{name}");
            programs.insert(program);
        }
        assert_eq!(programs.len(), CHOICE_SEEDS);
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    /// The seeds are numbered, so one of a program that left the
    /// repository stayed in the kept corpus under a number nothing
    /// overwrites any more.
    #[test]
    fn the_seeds_of_an_earlier_run_are_removed_and_what_was_found_stays() {
        let dir = corpus_of("fuzz_compiler", "stale");
        let source = dir.parent().unwrap().join("a.st");
        std::fs::write(&source, "FUNCTION f : INT\nEND_FUNCTION\n").unwrap();
        std::fs::write(dir.join("seed-000000.st"), "PROGRAM old\nEND_PROGRAM\n").unwrap();
        std::fs::write(dir.join("seed-004200.st"), "PROGRAM gone\nEND_PROGRAM\n").unwrap();
        std::fs::write(dir.join("crash-seed-of-nothing"), "x").unwrap();

        assert_eq!(remove_seeds(&dir).unwrap(), 2);
        assert_eq!(create_choice_seeds(&dir).unwrap(), None);
        assert_eq!(create_seed_corpus(&source, &dir, 0).unwrap(), 1);

        assert_eq!(names(&dir), ["crash-seed-of-nothing", "seed-000000.st"]);
        assert_eq!(
            std::fs::read_to_string(dir.join("seed-000000.st")).unwrap(),
            "FUNCTION f : INT\nEND_FUNCTION\n"
        );
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

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
