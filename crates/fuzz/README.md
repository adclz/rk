# IEC ST Compiler Fuzzer

Fuzzing infrastructure for testing the IEC ST compiler using [libfuzzer](https://llvm.org/docs/LibFuzzer/).

## Targets

- **fuzz_compiler** runs the whole pipeline: check, lint, lower, emit both profiles, validate, run.
- **fuzz_formatter** checks that the formatter's output parses, is stable, and means what the input meant.
- **fuzz_incremental** replays an edit the way the editor sends it, and compares the result with a fresh build.

> [!NOTE]  
> The parser and lexer are fuzzed separately through the tree-sitter CLI (via `tree-sitter fuzz`[https://tree-sitter.github.io/tree-sitter/cli/fuzz.html]).

The nightly [fuzzing workflow](../../.github/workflows/fuzzing.yml) runs each target for 30 minutes.

## Quick Start

### Generate Corpus

```bash
cargo run --release -p rk-fuzz --bin corpus_gen -- \
  crates/tree-sitter/test/corpus stdlib crates/doc/examples docs skills src/tests \
  crates/fuzz/corpus/fuzz_compiler
```

### Run Fuzzer

The targets are built with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz), which instruments them for coverage.
A plain `cargo build` gives libFuzzer nothing to steer by.

```bash
cargo +nightly fuzz run --fuzz-dir crates/fuzz --debug-assertions fuzz_compiler \
  crates/fuzz/corpus/fuzz_compiler -- -max_total_time=1800 -dict=crates/fuzz/st.dict
```

> [!WARNING]
> The ASan build covers the whole workspace, wasmtime included, and needs a lot of memory.
> Prefer the nightly workflow to a local run.

### Reproduce a Crash

`repro` runs every oracle on files or directories, on the stable toolchain, and names the one that fails:

```bash
cargo run -p rk-fuzz --bin repro -- crates/fuzz/artifacts/fuzz_compiler/crash-...
cargo run -p rk-fuzz --bin repro -- --only format my_test.st
```

It also tells you how far the compiler got (rejected by check, or compiled and ran).
Use it to check a hypothesis: write the program that should break the compiler and run it.

## What Gets Tested

- **Lowering**: a source `rk check` accepts must lower to MIR.
- **Wasm validation**: both profiles validate against the features rk claims, the ones `rk compile -O` enables in wasm-opt.
- **Profiles**: the debug and release modules have the same imports, globals, code and data.
- **Custom sections**: every section decodes at the current version, and the retain and located maps lie inside their bands.
- **Execution**: the module instantiates and runs `__init`, a few scans and its `{test}` and `{export}` functions under a fuel budget. An out-of-bounds access, an `unreachable` or a stack overflow is a bug; a RAISE or a division by zero is not.
- **Formatter**: the output parses, formatting twice changes nothing, and an accepted program keeps its diagnostics.
- **Incremental**: after the edit, the text, the syntax tree, the diagnostics (positions included) and the emitted module match a fresh build.

## Findings

- **`findings/`** holds the open bugs, one reproducer each, named `<oracle>--<what>.st`.
- **`regressions/`** holds the fixed ones.

`tests/findings.rs` checks that every open finding still fails the way its name says, and that every regression passes.
When you fix a bug, its test fails and tells you to move the file to `regressions/`.

## Corpus

`corpus_gen` takes every piece of ST in the repository: the tree-sitter tests, the standard library, the `iecst` fences of the docs and examples, and the programs in the Rust tests.
The codegen tests' programs compile and run, so they are the seeds that reach the code generator.

## References

- [libFuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [Rust Fuzzing Book](https://rust-fuzz.github.io/)
