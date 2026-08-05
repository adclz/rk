#![recursion_limit = "256"]
// `configure_parser!` (auto-lsp) expands to functions returning
// `Result<_, ParseError>` whose Err variant is ~144 bytes
#![allow(clippy::result_large_err)]
pub mod generated;
use crate::generated::SourceFile;
use auto_lsp::configure_parser;

configure_parser!(
    RK_PARSER,
    language: tree_sitter_rk::LANGUAGE,
    ast_root: SourceFile,
);
