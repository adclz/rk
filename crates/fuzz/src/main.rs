//! Corpus generator:
//! `cargo run --release --bin corpus_gen -- <source_dir> <output_dir>`.

use rk_fuzz::create_seed_corpus;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 3 {
        eprintln!("IEC ST Corpus Generator");
        eprintln!();
        eprintln!("Usage: {} <source_dir> <output_dir>", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <source_dir>   Directory containing tree-sitter test files (.txt)");
        eprintln!("  <output_dir>   Directory where corpus files will be written");
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} crates/tree-sitter/test/corpus crates/fuzz/corpus",
            args[0]
        );
        std::process::exit(1);
    }

    let source_dir = PathBuf::from(&args[1]);
    let output_dir = PathBuf::from(&args[2]);

    if !source_dir.exists() {
        eprintln!(
            "Error: source directory does not exist: {}",
            source_dir.display()
        );
        std::process::exit(1);
    }

    if !source_dir.is_dir() {
        eprintln!(
            "Error: source path is not a directory: {}",
            source_dir.display()
        );
        std::process::exit(1);
    }

    eprintln!(
        "Generating corpus from {} to {}",
        source_dir.display(),
        output_dir.display()
    );

    match create_seed_corpus(&source_dir, &output_dir) {
        Ok(count) => {
            eprintln!("✓ Successfully created {} corpus entries", count);
            if count == 0 {
                eprintln!("Warning: No corpus entries were extracted.");
                eprintln!("Make sure the source directory contains .txt files from tree-sitter.");
            }
        }
        Err(e) => {
            eprintln!("✗ Error creating corpus: {}", e);
            std::process::exit(1);
        }
    }
}
