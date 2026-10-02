// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

#![recursion_limit = "256"]
// `configure_parser!` (auto-lsp) expands to functions returning
// `Result<_, ParseError>` whose Err variant is ~144 bytes
#![allow(clippy::result_large_err)]
// Rewritten by build.rs on every build: formatting it would not last.
#[rustfmt::skip]
pub mod generated;
use crate::generated::SourceFile;
use auto_lsp::configure_parser;

configure_parser!(
    RK_PARSER,
    language: tree_sitter_rk::LANGUAGE,
    ast_root: SourceFile,
);
