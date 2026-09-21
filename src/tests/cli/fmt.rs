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
