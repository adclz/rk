// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What the fuzzers found, kept where the suite runs it.
//!
//! A unit test, not an integration one: Cargo builds a package's binaries
//! before its integration tests, and the `#![no_main]` fuzz targets only
//! link where libFuzzer provides `main`, which is not Windows.
//!
//! `findings/` holds the open bugs, one reproducer each, named
//! `<oracle>--<what>.st` (`panic` for a crash). Each must still fail the
//! way its name says: when one passes, the bug is fixed, and the file moves
//! to `regressions/`, where every input must pass every oracle for good.

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

fn inputs(dir: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    // Git keeps no empty directory: a missing one records nothing.
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "st"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let source = std::fs::read_to_string(&p).expect("a UTF-8 reproducer");
            (name, source)
        })
        .collect();
    out.sort();
    out
}

/// The oracle an input breaks: `pass`, `panic`, or a finding's oracle.
fn outcome(source: &str) -> String {
    match panic::catch_unwind(AssertUnwindSafe(|| crate::check_all(source))) {
        Ok(Ok(_)) => "pass".to_string(),
        Ok(Err(finding)) => finding.oracle.to_string(),
        Err(_) => "panic".to_string(),
    }
}

#[test]
fn past_findings_stay_fixed() {
    let broken: Vec<String> = inputs("regressions")
        .into_iter()
        .filter_map(|(name, source)| match outcome(&source).as_str() {
            "pass" => None,
            oracle => Some(format!("  {name}: [{oracle}]")),
        })
        .collect();
    assert!(
        broken.is_empty(),
        "fixed findings are failing again; `cargo run -p rk-fuzz --bin repro -- \
         crates/fuzz/regressions` says why:\n{}",
        broken.join("\n")
    );
}

#[test]
fn open_findings_still_reproduce() {
    let changed: Vec<String> = inputs("findings")
        .into_iter()
        .filter_map(|(name, source)| {
            let expected = name.split("--").next().unwrap_or_default().to_string();
            let actual = outcome(&source);
            (actual != expected).then(|| match actual.as_str() {
                "pass" => format!("  {name}: passes now; move it to crates/fuzz/regressions/"),
                _ => format!("  {name}: now fails as [{actual}]; rename it or fix the new cause"),
            })
        })
        .collect();
    assert!(
        changed.is_empty(),
        "open findings changed:\n{}",
        changed.join("\n")
    );
}
