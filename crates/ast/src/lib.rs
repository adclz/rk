#![recursion_limit = "256"]
pub mod generated;
use crate::generated::SourceFile;
use auto_lsp::configure_parsers;

configure_parsers!(
    RK_PARSER,
    "st" => {
        language: tree_sitter_rk::LANGUAGE,
        ast_root: SourceFile
    }
);
