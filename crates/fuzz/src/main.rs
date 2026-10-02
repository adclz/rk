// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Corpus generator:
//! `cargo run --release -p rk-fuzz --bin corpus_gen -- <source>... <output_dir>`.
//!
//! Each source is a file or a directory walked recursively; see
//! `rk_fuzz::corpus` for what is taken from which kind of file.

use rk_fuzz::create_seed_corpus;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("IEC ST Corpus Generator");
        eprintln!();
        eprintln!("Usage: {} <source>... <output_dir>", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <source>       A file or directory: tree-sitter tests (.txt), ST (.st),");
        eprintln!("                 Markdown with iecst fences (.md), Rust tests (.rs)");
        eprintln!("  <output_dir>   Directory where corpus files will be written");
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} crates/tree-sitter/test/corpus stdlib src/tests crates/fuzz/corpus",
            args[0]
        );
        std::process::exit(1);
    }

    let (sources, output) = args[1..].split_at(args.len() - 2);
    let output_dir = PathBuf::from(&output[0]);

    let mut total = 0;
    for source in sources {
        let source = PathBuf::from(source);
        if !source.exists() {
            eprintln!("Error: source does not exist: {}", source.display());
            std::process::exit(1);
        }
        match create_seed_corpus(&source, &output_dir, total) {
            Ok(count) => {
                eprintln!("{count:>6} entries from {}", source.display());
                total += count;
            }
            Err(e) => {
                eprintln!("Error reading {}: {e}", source.display());
                std::process::exit(1);
            }
        }
    }
    eprintln!("{total:>6} entries in {}", output_dir.display());
    if total == 0 {
        eprintln!("Warning: no corpus entries were extracted.");
        std::process::exit(1);
    }
}
