//! Every assertion of `Std.Unit` must fail when it should.
//!
//! The standard library's own suite only ever calls them on values that
//! agree, so an overload that never raised would make every test using its
//! type pass, and nothing would notice. An ST test cannot expect a failure,
//! so this one runs from here, on what `rk test` runs: the real `stdlib/`,
//! `init_db`, `build_core`, the CLI's test host.

use std::collections::{BTreeMap, BTreeSet};

use debug_format::test_report::TestRecord;
use rk::cli::OutputFormat;

use super::{temp_workspace, unset_stdlib};

/// Two values per type that differ, as initializers.
///
/// Where a 64-bit type counts finely enough (integers, bit strings and the
/// nanosecond types) the two share their low 32 bits, and the two LREALs are
/// one REAL: an overload that compared on the narrower lane would call them
/// equal. An LDATE counts days, so its two are simply different.
const SAMPLES: &[(&str, &str, &str)] = &[
    ("BOOL", "TRUE", "FALSE"),
    ("SINT", "SINT#-128", "SINT#127"),
    ("INT", "INT#-32768", "INT#32767"),
    ("DINT", "DINT#-2147483648", "DINT#2147483647"),
    ("LINT", "LINT#1", "LINT#4294967297"),
    ("USINT", "USINT#0", "USINT#255"),
    ("UINT", "UINT#0", "UINT#65535"),
    ("UDINT", "UDINT#1", "UDINT#4294967295"),
    ("ULINT", "ULINT#1", "ULINT#4294967297"),
    ("BYTE", "BYTE#16#01", "BYTE#16#81"),
    ("WORD", "WORD#16#0001", "WORD#16#8001"),
    ("DWORD", "DWORD#16#0000_0001", "DWORD#16#8000_0001"),
    ("LWORD", "LWORD#16#1", "LWORD#16#1_0000_0001"),
    ("REAL", "REAL#1.0", "REAL#1.0000001"),
    ("LREAL", "LREAL#1.0", "LREAL#1.0000000000000002"),
    ("TIME", "T#1s", "T#1s1ms"),
    ("LTIME", "LT#1ns", "LT#4s294ms967us297ns"),
    ("STRING", "'abc'", "'abd'"),
    ("CHAR", "CHAR#'a'", "CHAR#'b'"),
    ("DATE", "D#2024-02-29", "D#2024-03-01"),
    ("LDATE", "LD#2024-02-29", "LD#2024-03-01"),
    ("TIME_OF_DAY", "TOD#00:00:01", "TOD#00:00:02"),
    (
        "LTIME_OF_DAY",
        "LTOD#00:00:00.000000001",
        "LTOD#00:00:04.294967297",
    ),
    (
        "DATE_AND_TIME",
        "DT#1970-01-01-00:00:00",
        "DT#1970-01-01-00:00:01",
    ),
    (
        "LDATE_AND_TIME",
        "LDT#1970-01-01-00:00:00.000000001",
        "LDT#1970-01-01-00:00:04.294967297",
    ),
];

/// The type each overload of `function` compares, read from `Unit.st` itself,
/// so an overload added there without samples here fails this test.
fn overload_types(unit: &str, function: &str) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    let mut lines = unit.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line != format!("FUNCTION {function}") {
            continue;
        }
        let value = lines
            .by_ref()
            .find_map(|l| l.strip_prefix("value:"))
            .unwrap_or_else(|| panic!("an overload of {function} has no `value`"));
        types.insert(value.trim().trim_end_matches(';').to_string());
    }
    types
}

/// One `{test}` per overload and per outcome. The operands are variables of
/// the overload's own type, so resolution cannot land on a neighbour.
fn workspace_source() -> String {
    let mut src = String::from("USING Std.Unit;\n");
    let mut test = |name: &str, ty: &str, a: &str, b: &str, call: &str| {
        src.push_str(&format!(
            "\n{{test}}\nFUNCTION {name}\nVAR\n    a : {ty} := {a};\n    b : {ty} := {b};\nEND_VAR\n    \
             {call}(value := a, target := b, message := '{ty}');\nEND_FUNCTION\n"
        ));
    };
    for (ty, a, b) in SAMPLES {
        test(&format!("eq_{ty}_differs_FAILS"), ty, a, b, "ASSERT_EQ");
        test(&format!("eq_{ty}_same_passes"), ty, a, a, "ASSERT_EQ");
        test(&format!("neq_{ty}_same_FAILS"), ty, a, a, "ASSERT_NEQ");
        test(&format!("neq_{ty}_differs_passes"), ty, a, b, "ASSERT_NEQ");
    }
    src.push_str(
        "\n{test}\nFUNCTION assert_false_FAILS\n    ASSERT(FALSE, 'plain');\nEND_FUNCTION\n\
         \n{test}\nFUNCTION assert_true_passes\n    ASSERT(TRUE, 'plain');\nEND_FUNCTION\n",
    );
    src
}

/// One test function, because loading the standard library is what costs:
/// every overload rides the same build.
#[test]
fn every_assertion_fails_when_it_should_and_only_then() {
    let unit = include_str!("../../../stdlib/Unit.st");
    let sampled: BTreeSet<String> = SAMPLES.iter().map(|(ty, ..)| ty.to_string()).collect();
    for function in ["ASSERT_EQ", "ASSERT_NEQ"] {
        assert_eq!(
            overload_types(unit, function),
            sampled,
            "the overloads of {function} in Unit.st and the samples here must name the same types"
        );
    }

    // The library beside this binary: the checkout's `stdlib/`.
    unset_stdlib();
    let (_ws, root) = temp_workspace(&[("main.st", &workspace_source())]);
    let db = rk::workspace::init_db(&root, false, true).expect("the workspace loads");
    let (wasm, _) = rk::compiler::build_core_with_format(&db, &root, false, OutputFormat::Concise)
        .unwrap_or_else(|report| panic!("the workspace must compile:\n{report}"));

    let records: BTreeMap<String, TestRecord> = rk::test_host::run_each(&wasm, None, None, |_| {})
        .expect("the tests run")
        .into_iter()
        .map(|record| (record.name.clone(), record))
        .collect();
    assert_eq!(
        records.len(),
        SAMPLES.len() * 4 + 2,
        "every test ran: {:?}",
        records.keys()
    );

    let mut wrong = Vec::new();
    for (name, record) in &records {
        let must_fail = name.ends_with("_FAILS");
        match (must_fail, record.reason.as_deref()) {
            (false, None) => {}
            // The message proves the assertion raised, and carried what it was given.
            (true, Some(reason)) if reason.starts_with("assertion failed: ") => {}
            (true, None) => wrong.push(format!("{name}: passed, the assertion never raised")),
            (_, Some(reason)) => wrong.push(format!("{name}: {reason}")),
        }
    }
    assert!(
        wrong.is_empty(),
        "{} assertion(s) misbehave:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}
