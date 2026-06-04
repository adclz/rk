#![recursion_limit = "256"]
pub mod generated;
use crate::generated::SourceFile;
use auto_lsp::configure_parser;

configure_parser!(
    RK_PARSER,
    language: tree_sitter_rk::LANGUAGE,
    ast_root: SourceFile,
);
