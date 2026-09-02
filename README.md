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

# 2. Install JS dependencies, build the extension client, and package as .vsix
cd vscode
npm install && npm run build
npx @vscode/vsce package --target linux-x64
```

### Install

```bash
code --install-extension iec-st-linux-x64-1.0.0.vsix
```

### Development

The extension always runs `vscode/server/bin/vscode-lsp-server`; the F5 build
task copies the debug binary there. From that path the server finds the
checkout's own `stdlib/`, so a development run needs no library set up.
Launch via the VSCode debug configuration:

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

## Website

The site is the `skills/` directory rendered, plus the diagnostics reference,
built by one Rust binary that refuses to publish an example the compiler
disagrees with:

```bash
cargo run --release -p doc -- site/dist
# Output: site/dist (static), and crates/doc/diagnostics.json refreshed —
# `rk explain` embeds that file, so commit it when it changes.
```

Every `iecst` fence in a skill is checked; `crates/doc/src/skills.rs` lists
the fence markers (`fragment`, `decl`, `continues`, `syntax`, `sketch`,
`expect=`). `site/` holds the Cloudflare Worker that serves `site/dist`,
answers `Accept: text/markdown`, and hosts the read-only MCP server at `/mcp`:

```bash
cd site && npm ci && npm run check     # type-check + wrangler dry run
npm run dev                            # http://localhost:8788
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
