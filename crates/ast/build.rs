// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp_codegen::generate;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../tree-sitter/src/node-types.json");

    let output_path = PathBuf::from("./src/generated.rs");

    // The lines every Rust source under the AGPL starts with
    // (scripts/spdx.py); this one is written here, so they are too.
    let header = "// SPDX-FileCopyrightText: 2026 Clauzel Adrien\n\
                  // SPDX-License-Identifier: AGPL-3.0-only\n\n";
    let code = generate(
        tree_sitter_rk::NODE_TYPES,
        &tree_sitter_rk::LANGUAGE.into(),
        Some(HashMap::from([
            ("\n", "EOL"),
            (" ", "WHITESPACE"),
            ("`", "BACKTICK"),
        ])),
    )
    .to_string();
    fs::write(output_path, format!("{header}{code}")).unwrap();
}
