# IEC ST Compiler Fuzzer

Fuzz testing infrastructure for the IEC ST compiler using libfuzzer.

## How It Works

The fuzzer tests the entire compilation pipeline by:
1. Creating a fresh database instance
2. Parsing input as IEC ST source code
3. Running semantic analysis (type checking, name resolution, diagnostics)
4. Catching any panics or crashes

The fuzzer is designed to find crashes and bugs, not to verify correctness. Invalid syntax and semantic errors are expected and handled gracefully.

## Corpus Generation

The fuzzer uses a seed corpus extracted from tree-sitter test files. Each test file contains IEC ST code blocks in this format:

```
================================================================================
Test Title
================================================================================

SOURCE CODE HERE
(can be multiple lines)

----------------

(expected parse tree - ignored by fuzzer)
```

The `corpus_gen` tool extracts all code blocks and generates individual test cases.

### Generate Corpus

```bash
cargo run --release --manifest-path crates/fuzz/Cargo.toml --bin corpus_gen -- \
  crates/tree-sitter/test/corpus \
  crates/fuzz/corpus
```

This extracts ~150 test cases from the tree-sitter corpus files into `crates/fuzz/corpus/`.

## Building and Running

### Prerequisites

```bash
rustup install nightly
```

### Build Fuzzer

```bash
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_compiler
```

Binary: `target/release/fuzz_compiler`

### Run Fuzzer

Basic run (indefinite):
```bash
./target/release/fuzz_compiler crates/fuzz/corpus
```

Time-limited run (50 seconds):
```bash
./target/release/fuzz_compiler crates/fuzz/corpus -max_total_time=50
```

Reproduce a crash:
```bash
./target/release/fuzz_compiler crash-abc123def456
```

## Output Example

```
#0      READ units: 1/147 exec/s: 0 rss: 52Mb
#100    INITED cov: 450 ft: 1240 corp: 25/512b exec/s: 50 rss: 65Mb
#500    NEW    cov: 475 ft: 1350 corp: 35/892b exec/s: 100 rss: 72Mb
```

- `#N`: Iteration number
- `cov`: Code coverage points
- `corp`: Corpus size
- `exec/s`: Executions per second
- `rss`: Memory usage

## What Gets Tested

✅ Parser (tree-sitter integration)
✅ Semantic analysis (type checking, name resolution)
✅ Diagnostics generation
✅ Database queries (Salsa)
✅ Error handling

❌ Tree-sitter itself (has dedicated fuzzing)

## Performance

- Speed: ~65,000 executions per second
- Memory: ~35MB baseline
- Stability: Runs indefinitely without false positives

## Troubleshooting

### "Corpus is empty"
Regenerate it:
```bash
cargo run --release --manifest-path crates/fuzz/Cargo.toml --bin corpus_gen -- \
  crates/tree-sitter/test/corpus crates/fuzz/corpus
```

### Slow execution
Ensure you're using release build and nightly Rust.

### Fuzzer hangs
Check that corpus files exist and are readable:
```bash
ls crates/fuzz/corpus/ | head
```

## References

- [libFuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [Rust Fuzzing Book](https://rust-fuzz.github.io/)