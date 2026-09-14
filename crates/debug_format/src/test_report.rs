//! What a test run reports: one record per `{test}` function and a tally,
//! as newline-delimited JSON, shared by both sides of a process boundary.
//! A stream, not a document: tests first, a [`Summary`] last, so a reader
//! renders each test as it finishes. Self-describing, unlike the msgpack
//! sections: adding a field is backward-compatible.

use serde::{Deserialize, Serialize};

/// One line of a test report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReportLine {
    /// One test finished.
    Test(TestRecord),
    /// The run finished; always last.
    Summary(Summary),
}

/// Whether a test passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
}

/// One test and how it went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRecord {
    /// The test's fully qualified path, as the module's manifest records it.
    pub name: String,
    pub status: Status,
    /// Why it failed — an assertion message, or a trap and where it happened.
    /// Absent on a pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub duration_us: u64,
    /// Where the test was declared, when the module's manifest records it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

impl TestRecord {
    pub fn passed(&self) -> bool {
        self.status == Status::Pass
    }
}

/// The tally, emitted once at the end; `total == 0` means the module
/// declares no tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(status: Status) -> TestRecord {
        TestRecord {
            name: "Std.Math.abs_negates".into(),
            status,
            reason: (status == Status::Fail).then(|| "expected 3, got 4".to_string()),
            duration_us: 812,
            file: Some("math.st".into()),
            line: Some(42),
        }
    }

    /// The tag lets a reader stream mixed lines, so it is pinned.
    #[test]
    fn lines_are_tagged_by_type() {
        let test = serde_json::to_string(&ReportLine::Test(record(Status::Pass))).unwrap();
        assert!(test.starts_with(r#"{"type":"test","#), "{test}");
        let summary = serde_json::to_string(&ReportLine::Summary(Summary {
            total: 1,
            passed: 1,
            failed: 0,
        }))
        .unwrap();
        assert_eq!(
            summary,
            r#"{"type":"summary","total":1,"passed":1,"failed":0}"#
        );
    }

    /// A pass carries no reason field at all, rather than a null or an empty
    /// string — a reader that prints `reason` must not print "".
    #[test]
    fn a_pass_omits_the_reason() {
        let json = serde_json::to_string(&record(Status::Pass)).unwrap();
        assert!(!json.contains("reason"), "{json}");
        let json = serde_json::to_string(&record(Status::Fail)).unwrap();
        assert!(json.contains(r#""reason":"expected 3, got 4""#), "{json}");
    }

    /// Both sides are separate processes, so the round trip is the contract.
    #[test]
    fn lines_round_trip() {
        for line in [
            ReportLine::Test(record(Status::Fail)),
            ReportLine::Test(record(Status::Pass)),
            ReportLine::Summary(Summary {
                total: 2,
                passed: 1,
                failed: 1,
            }),
        ] {
            let json = serde_json::to_string(&line).unwrap();
            assert_eq!(
                serde_json::from_str::<ReportLine>(&json).unwrap(),
                line,
                "{json}"
            );
        }
    }

    /// Fields are named, so a reader built before a field existed still
    /// parses a newer line.
    #[test]
    fn an_unknown_field_does_not_break_a_reader() {
        let json = r#"{"type":"test","name":"t","status":"pass","duration_us":5,"invented":true}"#;
        let line: ReportLine = serde_json::from_str(json).expect("parses");
        match line {
            ReportLine::Test(t) => assert_eq!(t.name, "t"),
            _ => panic!("expected a test line"),
        }
    }
}
