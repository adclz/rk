use auto_lsp_codegen::generate;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=../tree-sitter/src/node-types.json");

    let output_path = PathBuf::from("./src/generated.rs");

    fs::write(
        output_path,
        generate(
            tree_sitter_rk::NODE_TYPES,
            &tree_sitter_rk::LANGUAGE.into(),
            Some(HashMap::from([
                ("\n", "EOL"),
                (" ", "WHITESPACE"),
                ("`", "BACKTICK")
            ]))
        )
        .to_string(),
    )
    .unwrap();
}