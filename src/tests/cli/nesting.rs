// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Blocks nested deep: what a chain of them costs, and the stack it takes.

use rk::cli::{Command, OutputFormat};

use super::{disable_stdlib, temp_workspace};

/// A chain of blocks each holding the next, checked and compiled as `rk`
/// runs its commands: on a thread with the stack the queries need, and a
/// pool with the same. Each block's layout, initializers and type are
/// computed once, where they were computed again under every block holding
/// one, and the queries nest as deep as the chain: a debug build's check
/// overflowed a pool thread's stack from 1,200 blocks.
#[test]
fn a_deep_chain_of_blocks_is_checked_and_compiled() {
    disable_stdlib();
    const DEPTH: usize = 1_500;
    const PER_FILE: usize = 100;
    let mut files: Vec<(String, String)> = Vec::new();
    for first in (0..DEPTH).step_by(PER_FILE) {
        let mut text = String::new();
        for i in first..(first + PER_FILE).min(DEPTH) {
            let inner = if i + 1 < DEPTH {
                format!("    inner : FB{};\n", i + 1)
            } else {
                String::new()
            };
            text.push_str(&format!(
                "FUNCTION_BLOCK FB{i}\nVAR\n    v : INT := {};\n{inner}END_VAR\nEND_FUNCTION_BLOCK\n\n",
                i % 1000
            ));
        }
        files.push((format!("n{:03}.st", first / PER_FILE), text));
    }
    files.push((
        "main.st".to_string(),
        "PROGRAM Main\nVAR\n    a : FB0;\nEND_VAR\nEND_PROGRAM\n\n\
         CONFIGURATION Plant\n    RESOURCE Res ON CPU\n        \
         TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);\n        \
         PROGRAM P WITH Fast : Main;\n    END_RESOURCE\nEND_CONFIGURATION\n"
            .to_string(),
    ));
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let (_ws, root) = temp_workspace(&files);

    let output = root.join("out.wasm");
    let ran = std::thread::Builder::new()
        .stack_size(db::STACK_SIZE)
        .spawn({
            let (root, output) = (root.clone(), output.clone());
            move || {
                let checked = rk::run(
                    &root,
                    false,
                    OutputFormat::Concise,
                    Command::Check { watch: false },
                );
                let built = rk::run(
                    &root,
                    false,
                    OutputFormat::Concise,
                    Command::Compile {
                        output: Some(output),
                        release: false,
                        opt_level: None,
                        debug: false,
                        watch: false,
                    },
                );
                (checked.is_ok(), built.is_ok())
            }
        })
        .unwrap()
        .join()
        .expect("the chain is checked and compiled without overflowing the stack");
    assert_eq!(ran, (true, true));
    assert!(output.exists(), "the module is written");
}
