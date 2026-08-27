# CLAUDE.md — AI Agent Guide for rk

## Project Overview

**rk** (codename **$RK**) is a compiler front-end and Language Server Protocol (LSP) implementation for **IEC 61131-3 Structured Text** (.st files). It is a demand-driven (query-based) compiler built on [Salsa](https://salsa-rs.netlify.app/) for incremental computation.

The architecture follows **CST → AST → HIR → Type Checking → IDE features**, similar to rust-analyzer.

- **Language**: Rust (edition 2024, toolchain pinned by rust-toolchain.toml)
- **License**: AGPL-3.0-only
- **Author**: Clauzel Adrien

## Build & Test Commands

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests (uses cargo-nextest)
cargo nextest run --workspace

# Run all tests without stopping on first failure
cargo nextest run --workspace --no-fail-fast

# Run a specific test
cargo test --package rk-tests --lib -- tests::semantics::array::valid_array --exact --nocapture

# Review insta snapshots after test changes
cargo insta review

# Build the tree-sitter grammar (run from crates/tree-sitter/)
tree-sitter generate

# Build the VSCode extension (run from vscode/)
npm run build

# Build the Rust LSP server binary for VSCode
cargo build --bin vscode-lsp-server

# Run the CLI diagnostic checker
cargo run --bin iec -- <workspace_path>

# Optimized builds (`rk compile -O <level>`) need a wasm-opt from Binaryen 119+
# on PATH; CI pins 131. The bundled `wasm-opt` CRATE is stuck at Binaryen 116
# (last released March 2024) which predates `try_table` — the instruction
# emitted for RAISE and the stdlib's assertions — so it cannot read any
# realistic module and `-O` degrades to an unoptimized (still correct) build
# with a warning.
#
# Use -O2/-O3/-Os/-Oz. `-O4` aborts on every Binaryen up to and including 131
# ("unexpected expr type" in the Flatten pass, which does not handle
# try_table) — an upstream limitation, not a stale version.

# Fuzz testing
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_compiler
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_formatter
```

## Workspace Structure

### Crate Dependency Graph

```
vscode/server (binary) — thin wrapper, logging setup
    └── server (LSP wiring, capability registration)
            ├── ide_proto (IDE feature implementations, HIR walking)
            │       ├── hir (HIR definitions, type system, diagnostics, name resolution)
            │       │       ├── ast (auto-generated from tree-sitter grammar)
            │       │       └── db (Salsa incremental database, file management)
            │       └── formatter (Topiary-based code formatter)
            └── ide_diagnostic (rich diagnostic data model)

cli (binary) — standalone diagnostic checker
    ├── hir, ast, db, ide_diagnostic
```

### Crate Purposes

| Crate                     | Path                    | Purpose                                                                                                                 |
| ------------------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `ast`                     | `crates/ast`            | **Auto-generated** AST types from tree-sitter grammar. Do NOT edit `generated.rs`.                                      |
| `tree-sitter-rk` | `crates/tree-sitter`    | Tree-sitter grammar for IEC 61131-3 (.st). Grammar source is `grammar.js`.                                              |
| `db`                      | `crates/db`             | Root Salsa database (`RootDatabase`), file management, workspace configuration.                                         |
| `hir`                     | `crates/hir`            | **Core of the compiler**: HIR definitions, semantic index, type inference, name resolution, diagnostics.                |
| `ide_proto`               | `crates/ide_proto`      | IDE feature implementations: hover, completions, go-to-definition, document symbols, inlay hints, semantic tokens, etc. |
| `ide_diagnostic`          | `crates/ide_diagnostic` | Rich diagnostic model with related info, quick fixes, notes, and ariadne report rendering.                              |
| `server`                  | `crates/server`         | LSP server wiring: connects `auto-lsp` framework to IEC-specific handlers.                                              |
| `formatter`               | `crates/formatter`      | Code formatter using Topiary (tree-sitter-based, declarative query rules).                                              |
| `memory_usage`            | `crates/memory_usage`   | Utility for heap memory measurement.                                                                                    |
| `cli`                     | `crates/cli`            | Standalone CLI linter/checker binary for .st files.                                                                     |
| `fuzz`                    | `crates/fuzz`           | Fuzz testing targets for the compiler and formatter.                                                                    |
| `vscode-lsp-server`       | `vscode/server`         | VSCode extension LSP server binary (thin wrapper over `server` crate).                                                  |

### Root Package

The root `Cargo.toml` defines the `rk-tests` package which contains **integration tests** in `src/tests/`. This package depends on `auto-lsp`, `ast`, `db`, `hir`, and `ide_proto`.

## Architecture Details

### HIR (High-Level Intermediate Representation) — `crates/hir`

The HIR is the core data layer, organized into:

- **`hir_def/`** — Pure data definitions mirroring IEC 61131-3 constructs:
  - `pous/` — POUs: `Function`, `FunctionBlock`, `Class`, `Interface`, `DataType`, `MethodDecl`, `VariableDecl`
  - `expressions/` — `Expr`, `Stmt`, `Spec` (type specifications), `PathExpr`, `InitExpr`
  - `scope.rs` — `ScopeId` (salsa::tracked), `Scope`, `ScopeKind`
  - `semantic_index.rs` — `SemanticIndex` (central salsa query), `semantic_index()` function
  - `interned/` — `Ident` (salsa::interned), `NamespacePath`
  - `namespace.rs`, `program.rs`, `config.rs`, `using.rs`

- **`builder/`** — AST → HIR lowering. `SemanticIndexBuilder` walks the CST and builds the `SemanticIndex`.

- **`hir_ty/`** — Type system:
  - `ty.rs` — `Type` enum (~25 variants: Elementary, Struct, Array, Enum, Ref, Function, etc.)
  - `name_res.rs` — Global name indexes, `resolve_namespace_access`, `pou_names_res`
  - `def_map.rs` — `LocalDefMap` (per-scope variable/POU index)
  - `head/` — Signature and initialization inference
  - `body/` — Statement-level type inference
  - `infer/` — `Infer` trait, coercion, expression inference, implicit cast table
  - `resolver/` — Path resolution, function call resolution, visibility checks

- **`check/`** — Diagnostic collection:
  - `errors/` — Error types categorized by code ranges (E00xx–E10xx)
  - `check_duplicates.rs`, `check_recursion.rs`

- **`query_string/`** — Symbol search for IDE features (exact/fuzzy/prefix)

### Three-Phase Type Inference

Type checking proceeds in three memoized salsa queries:

1. **Signature** (`infer_signature`) — Resolves variable `Spec` → `Type`, return types, method inheritance
2. **Initialization** (`infer_initialization`) — Resolves initializer expressions
3. **Body** (`infer_body`) — Resolves statements, expressions, function calls, path expressions

### Key Patterns

- **Salsa everywhere**: `#[salsa::tracked]` structs, `#[salsa::interned]` identifiers, `#[salsa::input]` for DB. The `'db` lifetime is threaded through all signatures.
- **`Type::Never` contract**: When resolution fails, `Type::Never` is returned AND a diagnostic MUST be emitted at that point. Consumers of `Never` skip further error reporting to avoid cascading errors.
- **`#[return_ref]`** on salsa queries for zero-copy access.
- **`#[no_eq]`** on span-only fields so editing positions don't trigger recomputation.
- **`compact_str::CompactString`** instead of `String` for memory efficiency.
- **`rustc_hash::FxHashMap`** for performance-critical lookups.
- **`ordermap::OrderMap`** when insertion order matters (e.g., variable declarations for parameter ordering).
- **Scope-stable IDs**: `SemanticIndexBuilder` uses a monotonic counter (not raw AST IDs) for `ScopeId` values, enabling fine-grained incrementality.
- **Head vs body separation**: Changing a function body doesn't re-infer its signature.

### Error Code Categories

| Range | Category                 |
| ----- | ------------------------ |
| E00xx | Syntax errors            |
| E01xx | Duplicate definitions    |
| E02xx | Scope/resolution errors  |
| E03xx | Type system errors       |
| E04xx | Visibility/access errors |
| E05xx | Inheritance/OOP errors   |
| E06xx | Array errors             |
| E07xx | Enum errors              |
| E08xx | Subrange errors          |
| E09xx | Recursion errors         |
| E10xx | Control flow errors      |

### AST Crate — Auto-Generated

The `crates/ast/` crate is **entirely auto-generated**. Do NOT manually edit `src/generated.rs`.

- The `build.rs` reads `crates/tree-sitter/src/node-types.json` and generates Rust types via `auto-lsp-codegen`
- `src/lib.rs` just registers the parser using `configure_parsers!` macro, mapping `"structured_text"` → tree-sitter LANGUAGE + `SourceFile` root
- To regenerate: modify `grammar.js` in `crates/tree-sitter/`, run `tree-sitter generate`, then `cargo build` in `crates/ast/`

### Tree-Sitter Grammar

The grammar (`crates/tree-sitter/grammar.js`, ~1900 lines) follows the IEC 61131-3 standard tables and defines:
- Top-level declarations: FUNCTION, FUNCTION_BLOCK, CLASS, INTERFACE, PROGRAM, TYPE, NAMESPACE, CONFIGURATION, USING
- Variable sections: VAR, VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT, VAR_TEMP, VAR_GLOBAL, VAR_EXTERNAL
- Statements: assignment, IF, CASE, FOR, WHILE, REPEAT, RETURN, EXIT, CONTINUE
- Expressions: arithmetic, boolean, comparison, path expressions, function calls, THIS/SUPER
- Type specifications: elementary types, structs, enums, arrays, subranges, references
- Error recovery: `ERR_*` rules for common mistakes
- Ladder diagram and FBD are stubs

## Testing Conventions

### Framework

- **rstest** — For test fixtures and parameterization
- **insta** — For snapshot testing (always inline snapshots with `@`)
- **cargo-nextest** — Test runner (used in CI and locally)

### Test Location

All integration tests live in `src/tests/`:
- `semantics/` — Type checking, diagnostics, error reporting (~30 test modules)
- `lsp/` — LSP features: hover, document symbols, formatter, semantic tokens, inlay hints, implementations
- `completions/` — Completion items: body, head, call signatures, fly imports, field, using, query scope
- `codegen/` — WASM codegen + runtime execution tests (compile IEC → MIR → wasm, run under wasmtime/`runtime::Plc`): value passing, inout, retain, enums, strings, debug/monitoring, scheduling. Helpers (`compile_to_wasm[_checked]`, `compile_to_mir_and_wasm`, `execute_wasm`) live in `codegen/mod.rs`
- `mir/` — MIR structure/lowering assertions (exports, extern pragmas)

Crates keep only in-crate `#[cfg(test)]` unit tests for crate-private machinery (e.g. `wasm_codegen`'s `graft.rs`/`builtins.rs`, `mir/src/memory.rs`).

### Standard Test Pattern (Diagnostics)

```rust
#[rstest]
fn my_test(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
        END_VAR
            x := 'hello'; // type error
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
        [E0301] Error: type mismatch
        ...
    ");
}
```

Key helpers from `src/tests/utils.rs`:
- `with_db` — rstest fixture returning a fresh `RootDatabase`
- `add_sources(db, &[source1, source2])` — Parse and register multiple .st files
- `test_diagnostics(db, &[sources])` — Add sources → collect all diagnostics → render with ariadne (no color, ASCII charset) → return String
- `find_pou_with_name(db, file, name)` — Look up a POU by name
- `find_namespace_with_name(db, file, name)` — Look up a namespace by name

### Snapshot Conventions

- Always use **inline snapshots** (`@r"..."` or `@r#"..."#`)
- Empty snapshot `@r""` means "no errors expected" (valid code test)
- Diagnostic snapshots include ariadne-rendered output with error codes, source annotations, and carets
- LSP feature tests use `assert_debug_snapshot!` for structured responses
- After modifying tests, run `cargo insta review` to accept/reject snapshot changes

### Test Naming

- `valid_*` — Tests that code is accepted without errors
- `invalid_*` — Tests that errors are correctly reported
- Descriptive names matching the scenario (e.g., `implicit_cast_int_to_real`, `recursive_struct_detection`)

## VSCode Extension

Located in `vscode/`:
- `package.json` — Extension manifest, language ID `iecst`, file extension `.st`
- `client/src/extension.ts` — Spawns the Rust LSP binary, creates language client
- `server/` — Rust LSP server binary (built with cargo)
- `syntaxes/st.tmLanguage.json` — TextMate grammar for syntax highlighting

Server binary resolution: always `<extension>/server/bin/vscode-lsp-server`
(`.exe` on Windows). There is no debug/release branch in the extension — the
F5 build task copies the debug binary to that path, and packaging copies the
release one.

Standard library resolution: the server finds it relative to its own path, so
a packaged extension ships `server/lib/rk/std/` and a development run picks up
the checkout's `stdlib/`. `rk env` prints the resolved path and its origin.

## CI Workflows

| Workflow      | Trigger                                            | What it does                                              |
| ------------- | -------------------------------------------------- | --------------------------------------------------------- |
| `rust`        | Push/PR (ignoring .md/.js/.ts)                     | `cargo nextest run --workspace`                           |
| `tree-sitter` | Push/PR touching grammar files                     | `tree-sitter test` + `tree-sitter fuzz`                   |
| `fuzzing`     | Daily cron + PR touching fuzz/hir/db/ast/formatter | Builds and runs compiler+formatter fuzzers for 30min each |

## Key Dependencies

| Dependency     | Purpose                                                                                      |
| -------------- | -------------------------------------------------------------------------------------------- |
| `auto-lsp`     | LSP framework, Salsa integration, file management (custom fork at github.com/adclz/auto-lsp) |
| `salsa`        | Incremental computation framework (v0.26, must stay in sync with auto-lsp)                 |
| `tree-sitter`  | Parser generator, used via the grammar in `crates/tree-sitter/`                              |
| `topiary-core` | Tree-sitter-based code formatter engine                                                      |
| `ariadne`      | Pretty diagnostic report rendering (used in tests and CLI)                                   |
| `bon`          | Builder pattern macro                                                                        |
| `phf`          | Perfect hash maps (compile-time static maps)                                                 |
| `bitflags`     | For `Modifier` and `Visibility` flags                                                        |
| `compact_str`  | Small-string-optimized string type                                                           |
| `rustc-hash`   | Fast hash maps (`FxHashMap`)                                                                 |
| `ordermap`     | Insertion-order-preserving hash map                                                          |
| `rayon`        | Parallelism for workspace-wide operations                                                    |
| `fst`          | Finite state transducer (used in name resolution indexes)                                    |

## Important Notes

- The `salsa` version MUST stay in sync with `auto-lsp`'s salsa version (currently 0.26)
- The Rust toolchain is pinned by `rust-toolchain.toml` (stable; rustup applies it automatically). Only the fuzzers need nightly (`cargo +nightly`)
- `crates/ast/src/generated.rs` is auto-generated — never edit it manually
- Tree-sitter grammar changes require running `tree-sitter generate` before rebuilding
- Insta snapshots use ASCII charset and no color for deterministic output across environments
- The `auto-lsp` dependency is pinned to a specific git revision — update both `auto-lsp` and `auto-lsp-codegen` together
