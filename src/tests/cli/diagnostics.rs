use rk::diagnostics::{DiagnosticReporter, collect_diagnostics};

use super::{disable_stdlib, temp_workspace};

use rk::cli::OutputFormat;
use rk::workspace::init_db;

// One E0301 (type mismatch) with a quick-fix and related info — exercises
// every field the machine formats carry.
const SRC: &str = "FUNCTION f : INT\nVAR x : INT; END_VAR\n    x := ULINT#5;\nEND_FUNCTION\n";

/// Render the fixture workspace's diagnostics in `format`.
fn render(format: OutputFormat) -> String {
    let (_ws, root) = temp_workspace(&[("main.st", SRC)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let per_file = collect_diagnostics(&db, false);
    let mut out = Vec::new();
    let counts = DiagnosticReporter::new(&db, &root)
        .with_format(format)
        .report_files(&per_file, &mut out);
    assert!(
        counts.has_errors(),
        "fixture must produce at least one error"
    );
    String::from_utf8(out).unwrap()
}

/// `FILE:LINE:COL: severity[CODE]: message` — one line, digits where digits
/// belong, no ANSI ever (machine formats bypass the color pipeline).
#[test]
fn concise_is_one_regex_stable_line_per_diagnostic() {
    let out = render(OutputFormat::Concise);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1, "one diagnostic, one line: {out:?}");
    let line = lines[0];

    assert!(
        !line.contains('\x1b'),
        "no ANSI in concise output: {line:?}"
    );
    assert!(
        line.starts_with("main.st:"),
        "workspace-relative path: {line:?}"
    );
    let mut parts = line.splitn(4, ':');
    let (_file, l, c, rest) = (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    );
    l.parse::<u32>().expect("line is a number");
    c.parse::<u32>().expect("col is a number");
    assert!(
        rest.starts_with(" error[E0301]: "),
        "severity[CODE]: {rest:?}"
    );
    assert!(
        line.contains(": help: "),
        "quick-fix title appended: {line:?}"
    );
}

/// Every line parses as JSON with the stable field set; positions 1-based.
#[test]
fn json_lines_parse_with_stable_fields() {
    let out = render(OutputFormat::JsonLines);
    assert!(!out.contains('\x1b'), "no ANSI in json-lines output");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1, "one diagnostic, one JSON line");

    let v: serde_json::Value = serde_json::from_str(lines[0]).expect("valid JSON");
    assert_eq!(v["file"], "main.st");
    assert_eq!(v["severity"], "error");
    assert_eq!(v["code"], "E0301");
    assert_eq!(v["line"], 3, "1-based line of `x := ULINT#5`");
    assert!(v["col"].as_u64().unwrap() >= 1, "1-based column");
    assert!(
        !v["message"].as_str().unwrap().is_empty(),
        "message present"
    );
    assert!(
        v["help"].as_array().is_some_and(|h| !h.is_empty()),
        "quick-fix titles carried: {v}"
    );
}

/// A workspace whose only findings are lints must count zero errors;
/// `rk check` bases its exit code on `has_errors()`.
#[test]
fn advice_only_workspace_has_no_errors() {
    let (_ws, root) = temp_workspace(&[
        // `select = "all"` is load-bearing: `unused-variable` is info
        // severity, outside the recommended baseline.
        (
            "config.toml",
            "[project]\nname = \"T\"\nversion = \"0.0\"\n\n[linter]\nselect = \"all\"\n",
        ),
        // Valid code with an unused variable — a linter finding, not an error.
        (
            "main.st",
            "FUNCTION f : INT\nVAR unused : INT; END_VAR\n    f := 1;\nEND_FUNCTION\n",
        ),
    ]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let per_file = collect_diagnostics(&db, true);
    let mut out = Vec::new();
    let counts = DiagnosticReporter::new(&db, &root)
        .with_format(OutputFormat::Concise)
        .report_files(&per_file, &mut out);

    assert_eq!(counts.errors, 0, "no errors in an advice-only workspace");
    assert!(
        counts.infos + counts.hints > 0,
        "the unused-variable lint fired: {counts:?}"
    );
    assert!(!counts.has_errors(), "advice alone must not fail the check");
    let out = String::from_utf8(out).unwrap();
    assert!(
        out.lines()
            .all(|l| l.contains(": info") || l.contains(": hint") || l.contains(": warning")),
        "only advice lines rendered: {out:?}"
    );
}

/// A bare directory of `.st` files, no config.toml, is checkable;
/// artifact-producing commands keep requiring a config.
#[test]
fn no_config_workspace_is_checkable_with_full_analysis() {
    let ws = tempfile::tempdir().expect("tempdir");
    std::fs::write(ws.path().join("main.st"), SRC).unwrap();

    assert!(
        init_db(ws.path(), false, true).is_none(),
        "require_config must still refuse"
    );

    disable_stdlib();
    let db = init_db(ws.path(), false, false).expect("configless init");
    let per_file = collect_diagnostics(&db, true);
    let mut out = Vec::new();
    let counts = DiagnosticReporter::new(&db, ws.path())
        .with_format(OutputFormat::Concise)
        .report_files(&per_file, &mut out);

    assert_eq!(
        counts.errors, 1,
        "the real E0301 is still found: {counts:?}"
    );
    assert!(counts.hints >= 1, "the E1401 hint rides along: {counts:?}");
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("E0301"), "type error reported: {out}");
    assert!(
        out.contains("E1401"),
        "outside-a-project hint reported: {out}"
    );
}

/// The full format still renders the ariadne report (source excerpt +
/// header) — the machine formats must not have replaced it.
#[test]
fn full_still_renders_ariadne_reports() {
    let out = render(OutputFormat::Full);
    assert!(out.contains("E0301"), "code in the header: {out}");
    assert!(out.contains("main.st"), "workspace-relative path: {out}");
    assert!(out.contains("ULINT#5"), "source excerpt shown");
}
