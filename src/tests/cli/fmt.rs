// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use rk::cli::OutputFormat;
use rk::commands::fmt::run_fmt;

use super::temp_workspace;

/// A MISSING-node syntax error (E0002), the shape an editor's
/// format-on-save produces mid-edit, is refused.
#[test]
fn a_file_with_a_missing_node_is_left_untouched_and_counted_as_an_error() {
    let source = "FUNCTION f : INT\nVAR acc : INT; END_VAR\n    f := acc + INT#some_call(1, 2, 3);\nEND_FUNCTION\n\nFUNCTION g : INT  g := 2; END_FUNCTION\n";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    let result = run_fmt(&root, false, false, OutputFormat::Concise);
    assert!(result.is_err(), "a syntax error must fail the run");
    let after = std::fs::read_to_string(root.join("main.st")).unwrap();
    assert_eq!(after, source, "the file must be byte-identical");
}

/// The control: a clean file is still formatted and the run succeeds.
#[test]
fn a_clean_file_is_still_formatted() {
    let source = "FUNCTION g : INT  g := 2; END_FUNCTION\n";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    assert!(run_fmt(&root, false, false, OutputFormat::Concise).is_ok());
    let after = std::fs::read_to_string(root.join("main.st")).unwrap();
    assert_ne!(after, source, "the one-line function is reflowed");
}

/// A TASK written without the comma between two parameters is a syntax
/// error, and the file is left as it is. It was accepted, and the formatter
/// wrote the two parameters together, `T#10msPRIORITY := 1`: a file that
/// checked before `rk fmt` no longer parsed after it.
#[test]
fn a_task_without_its_comma_is_left_untouched() {
    let source = "PROGRAM Main\nEND_PROGRAM\n\nCONFIGURATION Cfg\n    RESOURCE Res ON CPU\n        TASK T(INTERVAL := T#10ms PRIORITY := 1);\n        PROGRAM P1 WITH T : Main;\n    END_RESOURCE\nEND_CONFIGURATION\n";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    let result = run_fmt(&root, false, false, OutputFormat::Concise);
    assert!(result.is_err(), "a syntax error must fail the run");
    let after = std::fs::read_to_string(root.join("main.st")).unwrap();
    assert_eq!(after, source, "the file must be byte-identical");
}

/// The same TASK with its comma is formatted, and what is written formats
/// to itself: the output of `rk fmt` is a file `rk fmt` takes.
#[test]
fn a_task_is_formatted_to_a_file_that_formats_to_itself() {
    let source = "PROGRAM Main\nEND_PROGRAM\n\nCONFIGURATION Cfg\n    RESOURCE Res ON CPU\n        TASK T(INTERVAL := T#10ms,PRIORITY := 1);\n        TASK U(INTERVAL := T#1s);\n        PROGRAM P1 WITH T : Main;\n    END_RESOURCE\nEND_CONFIGURATION\n";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    assert!(run_fmt(&root, false, false, OutputFormat::Concise).is_ok());
    let once = std::fs::read_to_string(root.join("main.st")).unwrap();
    assert!(
        once.contains("TASK T(INTERVAL := T#10ms, PRIORITY := 1);"),
        "{once}"
    );
    assert!(run_fmt(&root, false, false, OutputFormat::Concise).is_ok());
    let twice = std::fs::read_to_string(root.join("main.st")).unwrap();
    assert_eq!(once, twice);
}
