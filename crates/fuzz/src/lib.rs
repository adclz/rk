//! Fuzzing for the compiler: parsing, semantic analysis and diagnostics, to
//! catch panics rather than to verify correctness. Generate a seed corpus
//! with `cargo run --release --bin corpus_gen -- crates/tree-sitter/test/corpus
//! crates/fuzz/corpus`, build with `cargo +nightly build --release --bin
//! fuzz_compiler`, run `./target/release/fuzz_compiler crates/fuzz/corpus`
//! (`-max_total_time=300` bounds it; a crash file reproduces one).

pub mod corpus;

// Re-export for convenience
pub use corpus::{create_seed_corpus, load_corpus};
