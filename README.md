# rk

A compiler front-end and Language Server Protocol (LSP) implementation for **IEC 61131-3 Structured Text** (.st files).

Built as a demand-driven (query-based) compiler on [Salsa](https://salsa-rs.netlify.app/) for incremental computation.
Architecture follows **CST > AST > HIR > Type Checking > IDE features**, similar to rust-analyzer.

## Build

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo nextest run --workspace

# Run a specific test
cargo test --package rk-tests --lib -- tests::semantics::array::valid_array --exact --nocapture

# Review insta snapshots after test changes
cargo insta review
```

## VSCode Extension

### Prerequisites

- Rust toolchain
- Node.js + npm

### Build & Package

```bash
# 1. Build the Rust LSP server binary (release + strip)
cargo build --release --package vscode-lsp-server
cp target/release/vscode-lsp-server vscode/server/bin/
strip vscode/server/bin/vscode-lsp-server

# 2. Install JS dependencies and build the extension client
cd vscode && npm install && npm run build

# 3. Package as .vsix
npx @vscode/vsce package --target linux-x64
```

### Install

```bash
code --install-extension iec-st-linux-x64-1.0.0.vsix
```

### Development

For debug builds, the extension looks for `target/debug/vscode-lsp-server`. Launch via the VSCode debug configuration:

```bash
cargo build --bin vscode-lsp-server
cd vscode && npm install && npm run build
# Then press F5 in VSCode with the extension host launch config
```

## CLI

Standalone diagnostic checker for .st files:

```bash
cargo run --bin iec -- <workspace_path>
```

## Documentation Generator

Generates a single-page HTML diagnostic reference with all compiler errors and linter warnings:

```bash
cargo run --package doc --bin generate-docs
# Output: crates/doc/out/index.html
```

## Tree-Sitter Grammar

The grammar source is `crates/tree-sitter/grammar.js`. After modifying:

```bash
cd crates/tree-sitter
tree-sitter generate
# Then rebuild: cargo build -p ast
```

## Fuzz Testing

```bash
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_compiler
cargo +nightly build --release --manifest-path crates/fuzz/Cargo.toml --bin fuzz_formatter
```

## License

AGPL-3.0-only
