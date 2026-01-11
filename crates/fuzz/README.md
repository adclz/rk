# IEC ST Compiler Fuzzer

Fuzzing infrastructure for testing the IEC ST compiler using [libfuzzer](https://llvm.org/docs/LibFuzzer/).

## Targets

- **fuzz_compiler**: Tests the entire compilation pipeline (parse → semantic analysis → diagnostics)
- **fuzz_formatter**: Tests formatter idempotency (formatting twice produces identical output)

> [!NOTE]  
> The parser and lexer are fuzzed separately through the tree-sitter CLI (via `tree-sitter fuzz`[https://tree-sitter.github.io/tree-sitter/cli/fuzz.html]).


## Quick Start

### Generate Corpus

```bash
cd rk
cargo run --release --manifest-path crates/fuzz/Cargo.toml --bin corpus_gen -- \
  crates/tree-sitter/test/corpus \
  crates/fuzz/corpus
```

### Run Fuzzer

Compiler fuzzer (30 minutes):
```bash
cd rk
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_compiler
./target/release/fuzz_compiler crates/fuzz/corpus -max_total_time=1800
```

Formatter fuzzer (30 minutes):
```bash
cd rk
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_formatter
./target/release/fuzz_formatter crates/fuzz/corpus -max_total_time=1800
```

### Reproduce a Crash

```bash
./target/release/fuzz_compiler /path/to/crash-file
RUST_BACKTRACE=1 ./target/release/fuzz_compiler /path/to/crash-file
```

## What Gets Tested

- **fuzz_compiler**: Catches crashes, panics, and hangs in the compiler. Invalid syntax is handled gracefully.
- **fuzz_formatter**: Verifies that formatting is idempotent (formatting an already-formatted file doesn't change it).

## Corpus

The seed corpus is auto-generated from tree-sitter test files in `crates/tree-sitter/test/corpus/`. To add more test cases, edit those files and re-run `corpus_gen`.

## References

- [libFuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [Rust Fuzzing Book](https://rust-fuzz.github.io/)
