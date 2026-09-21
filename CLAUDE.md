# CLAUDE.md — AI Agent Guide for rk

## Project Overview

**rk** (codename **$RK**) is a compiler front-end and Language Server Protocol (LSP) implementation for **IEC 61131-3 Structured Text** (.st files). It is a demand-driven (query-based) compiler built on [Salsa](https://salsa-rs.netlify.app/) for incremental computation.

The architecture follows **CST → AST → HIR → Type Checking → IDE features**, similar to rust-analyzer.

- **Language**: Rust (edition 2024, toolchain pinned by rust-toolchain.toml)
- **License**: AGPL-3.0-only by default; LICENSING.md lists the Apache-2.0 directories (builtin bundle, stdlib, `debug_format`, tree-sitter grammar), the MIT ones (`index`, `macros`) and LICENSE-EXCEPTION for generated modules
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

# Build the website. The generator verifies every example against the
# compiler and writes site/content, site/data and site/static (gitignored);
# Zola renders them into site/dist. Also refreshes crates/doc/diagnostics.json,
# which `rk explain` embeds. Refuses to write when an example disagrees with
# the compiler. Release: the skills gate loads the stdlib once per example.
cd site && npm run build
# Preview with the Worker, as deployed: `cd site && npm run dev`.
#
# Use the npm scripts, not `zola build`, whenever a `wrangler dev` is up.
# Zola DELETES its output directory on every build; wrangler binds ./dist once
# and its assets die with that directory, answering 500 until it is restarted.
# `npm run sync` builds into site/.zola-out and rsyncs into dist, so dist keeps
# the inode wrangler holds and a reload just works. CI calls `zola build`
# straight, where nothing is watching.
#
# Authoring loop for site/pages/*.md and README.md. Zola watches content/,
# not pages/, so a page edit shows nothing until the generator runs again.
# --pages-only re-renders only the pages, against the last full run's derived
# values: about 1s instead of 14s. Their fences are still checked. It refuses
# until a full run has produced site/.substitutions.json, and CI never uses it.
cd site && npm run pages
#
# A dead `wrangler dev` leaves its workerd child holding the port, so a new one
# silently moves to 8788 and the old address keeps serving 500s:
#   pkill -f 'bin/workerd'
#
# CI pins Zola 0.23.6. The site uses no Zola shortcodes — the generator
# substitutes the derived HTML itself — so a Zola upgrade only has to keep
# the templates in `site/templates/` working.
#
# The front page IS README.md: the generator reads it, highlights its fences
# and points its repo-relative links at GitHub. Its examples are shown, never
# compiled, because several deliberately do not.
#
# The README's cast tables, between `<!-- casts:begin -->` and `:end`, are
# WRITTEN by the generator from `ElementarySpec::implicit_cast` and the
# `X_TO_Y` functions in stdlib/Convert.st; edit those, not the table. It also
# checks that every cast E0301 could suggest (`explicit_cast`) is a function
# that exists. CI diffs README.md after a run, like diagnostics.json.

# Regenerate THIRD-PARTY-NOTICES, the licenses of the crates compiled into
# every generated module (run from crates/wasm_builtins/; needs
# `cargo install cargo-about --locked --features cli`). CI diffs the result
# against the committed file.
cargo about generate about.hbs -o ../../THIRD-PARTY-NOTICES

# Build the tree-sitter grammar (run from crates/tree-sitter/)
tree-sitter generate

# Build the VSCode extension (run from vscode/)
npm run build

# Build the Rust LSP server binary for VSCode
cargo build --bin vscode-lsp-server

# Check a workspace from the CLI
cargo run --bin rk -- check --workspace <workspace_path>

# Optimized builds (`rk compile -O <level>`) need a wasm-opt from Binaryen 119+
# on PATH; CI pins 131. The bundled `wasm-opt` CRATE is stuck at Binaryen 116
# (last released March 2024) which predates `try_table` — the instruction
# emitted for RAISE and the stdlib's assertions — so it cannot read any
# realistic module and `-O` degrades to an unoptimized (still correct) build
# with a warning.
#
# `-O4` alone aborts on every Binaryen up to and including 131 ("unexpected
# expr type" in the Flatten pass, which does not handle try_table) — an
# upstream limitation, not a stale version. rk therefore runs it as
# `-O4 --skip-pass=flatten`, silently; the README carries the warning.
# Tracked as WebAssembly/binaryen#8372, where the maintainer has no near-term
# plan for it. Flatten does handle the LEGACY `try`, which is why "Binaryen
# supports exceptions" and "-O4 crashes" are both true. With the pass skipped
# what is left of -O4 is close to -O3 (192,121 bytes against 192,115 on the
# stdlib module). Trying Flatten first is not done: a debug-profile module
# (what `rk test -O` optimizes) always has the test wrappers' `try_table`, and
# a release one has it as soon as the program can raise.

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

cli (binary `rk`) — check, compile, test, fmt, explain, env
    ├── hir, ast, db, ide_diagnostic, linter, formatter, mir, wasm_codegen, debug_format
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
| `cli`                     | `crates/cli`            | The `rk` binary: check, compile, test (on an in-process wasmtime host), fmt, explain, env.                              |
| `mir`                     | `crates/mir`            | Mid-level IR between HIR and WASM codegen: layouts, lowering, the task schedule and retain map.                          |
| `wasm_codegen`            | `crates/wasm_codegen`   | Emits the core WASM module from MIR, with the builtin bundle and the debug/test custom sections.                        |
| `debug_format`            | `crates/debug_format`   | The custom-section formats (debug symbols, lines, schedule, retain map, test manifest) and their decoder.               |
| `linter`                  | `crates/linter`         | Lint rules (L-codes) over HIR.                                                                                          |
| `benchmark`               | `crates/benchmark`      | Divan benchmarks over the stdlib corpus, with diagnostic baselines.                                                     |
| `doc`                     | `crates/doc`            | Site generator's front half: verifies `skills/`, `crates/doc/examples/` and `site/pages/` against the compiler, pre-renders their code, and writes what Zola (`site/`) renders. |
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
  - `errors/` — Error types; the code's first two digits name its section (E00xx–E15xx, L00xx–L03xx)
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
- **Exports are opt-in**: a module exports `__init`, the PROGRAM bodies, the FUNCTIONs marked `{export}` and, in the debug profile only, the workspace's `{test}` functions. Everything else is `MirLinkage::Internal`, FB bodies and methods included, because an export is a root Binaryen cannot remove. A library's `{test}` functions are not lowered. The codegen tests call FUNCTIONs by name, so `codegen/harness.rs` re-exports everything (`export_everything`) on a clone of the MIR.

### Error Code Categories

| Range | Category                 |
| ----- | ------------------------ |
| E00xx | Syntax                   |
| E01xx | Duplicates               |
| E02xx | Resolution               |
| E03xx | Type System              |
| E04xx | Initializers             |
| E05xx | Arrays                   |
| E06xx | Enums                    |
| E07xx | Subranges                |
| E08xx | Calls                    |
| E09xx | References               |
| E10xx | Visibility               |
| E11xx | OOP                      |
| E12xx | Control Flow             |
| E13xx | Recursion                |
| E14xx | Configuration            |
| E15xx | Pragmas                  |
| L00xx | Lint pragmas             |
| L01xx | Linter Warning           |
| L02xx | Linter Info              |
| L03xx | Linter Hint              |

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
- `codegen/` — WASM codegen + execution tests (compile IEC → MIR → wasm, run on the wasmtime harness): value passing, inout, retain bands, enums, strings, debug symbols, scheduling. Helpers (`compile_to_wasm`, `compile_to_mir_and_wasm`, `execute_wasm`, `TestPlc`, `run_tests`) live in `codegen/harness.rs`
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
F5 build task and packaging both copy the release binary to that path: the
debug build answers a first diagnostic pull on the stdlib in ~27 s where the
release one takes 0.2 s.

Standard library resolution: the server finds it relative to its own path, so
a packaged extension ships `server/lib/rk/std/` and a development run picks up
the checkout's `stdlib/`. `rk env` prints the resolved path and its origin.

## CI Workflows

All on GitHub Actions, under `.github/workflows/`. Every Rust job installs the
toolchain pinned by `rust-toolchain.toml` through the composite action in
`.github/actions/rust-toolchain` and caches with `Swatinem/rust-cache`.

| Workflow      | Trigger                                              | What it does                                                                                                                                                                                                                                                                       |
| ------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ci`          | Push to main, PR, manual                             | `test`: `cargo nextest run --workspace --profile ci` on Linux, macOS and Windows. `clippy`: `-Dwarnings`. `stdlib`: the stdlib's own suite, plain and `-O z` on Binaryen 131. `notices`: `THIRD-PARTY-NOTICES` matches a fresh `cargo about` run. `site`: examples vs compiler, `diagnostics.json` freshness, Worker bundle |
| `tree-sitter` | Push/PR touching `crates/tree-sitter/**`             | `tree-sitter test` + `tree-sitter fuzz`                                                                                                                                                                                                                                            |
| `fuzzing`     | Daily at 02:00 UTC, manual                           | Builds and runs the compiler and formatter fuzzers for 30 min each; crashes are uploaded as artifacts and fail the run                                                                                                                                                             |
| `codspeed`    | Push to main, PR                                     | Benchmarks under CodSpeed                                                                                                                                                                                                                                                          |
| `site`        | Push to main touching skills, crates, stdlib or site | Builds the website and deploys the Cloudflare Worker                                                                                                                                                                                                                               |

### What keeps a pull request away from the deploy

`site.yml` is the only workflow that names a secret, and it runs on push to
`main` and manual dispatch only — never on `pull_request`, and nothing anywhere
uses `pull_request_target`. GitHub withholds repository secrets from any run
started by a forked pull request, so the workflows that do run on pull requests
have nothing to leak. A pull request also cannot run its own edited copy of
`site.yml`: workflow changes only take effect once merged.

Two habits keep that true. Every third-party action is pinned to a full commit
SHA, because a tag is mutable by its owner and one of these actions runs in the
job that holds the Cloudflare token; `.github/dependabot.yml` bumps those pins
weekly so they do not rot. And `npm ci --ignore-scripts` means a package's
install script never executes in that job, nor on a pull request, where the
lockfile is whatever the contributor wrote.

Four things live in the GitHub UI and no file here can enforce them:

| Setting | Wanted |
| --- | --- |
| Actions → Fork pull request workflows from outside collaborators | Require approval for all outside collaborators |
| Actions → Workflow permissions | Read repository contents permission |
| Rules → the `main` ruleset | Block force pushes and deletions, require a pull request, require the `ci` checks |
| The `CLOUDFLARE_API_TOKEN` secret | Scoped to Workers Scripts: Edit, on this account only |

`SITE_URL` is a repository variable, not a secret, and is baked into the
sitemap, the canonical tags, `llms.txt` and the MCP server card. Nothing
validates it: a wrong value builds a clean site that points everywhere at an
address that does not exist.

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
