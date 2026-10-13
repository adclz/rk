// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk compile --release -O <level>`: every level builds.

use rk::cli::{Command, OutputFormat};

use super::{disable_stdlib, temp_workspace};

/// A release build is taken at each level, `-O0` included. At that level
/// wasm-opt hands a module with no library code back byte for byte, which
/// read as "the optimizer did not run": the build was refused, and told to
/// install the Binaryen it had just run.
#[test]
fn a_release_build_is_taken_at_each_level() {
    disable_stdlib();
    if !crate::tests::utils::wasm_opt_installed() {
        crate::tests::utils::skip_without_wasm_opt("the release levels");
        return;
    }
    let source = "PROGRAM P
VAR n : DINT; END_VAR
    IF n < 10 THEN
        n := n + 1;
    END_IF;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Run WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    for level in ["0", "1", "2", "3", "4", "s", "z"] {
        let output = root.join(format!("out-{level}.wasm"));
        let built = rk::run(
            &root,
            false,
            OutputFormat::Concise,
            Command::Compile {
                output: Some(output.clone()),
                release: true,
                opt_level: Some(level.to_string()),
                debug: false,
                watch: false,
            },
        );
        assert!(built.is_ok(), "-O{level} builds");
        let module = std::fs::read(&output).expect("the module is written");
        wasm_codegen::validate(&module).unwrap_or_else(|e| panic!("-O{level} validates: {e}"));
    }
}
