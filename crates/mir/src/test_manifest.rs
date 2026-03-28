//! Test manifest — serialized metadata about test functions and their cases.
//!
//! Stored as a MessagePack-encoded custom section in the WASM module.
//! The test runner reads this to discover and execute tests.

use serde::{Deserialize, Serialize};

/// The complete test manifest for a module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestManifest {
    pub tests: Vec<TestEntry>,
}

/// A single test function entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestEntry {
    /// Fully qualified path (e.g., "Std.Math.Test.test_abs").
    pub path: String,

    /// WASM export name for the base test (no cases).
    pub export: String,

    /// Test cases (empty if no `{case}` pragmas).
    pub cases: Vec<TestCase>,
}

/// A parameterized test case from `{case(...)}` pragma.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestCase {
    /// WASM export name for this specific case.
    pub export: String,

    /// Arguments as typed values.
    pub args: Vec<TestValue>,
}

/// A typed value for a test case argument.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
}

impl Default for TestManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestManifest {
    pub fn new() -> Self {
        TestManifest { tests: Vec::new() }
    }

    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("TestManifest serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

impl std::fmt::Display for TestManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for test in &self.tests {
            if test.cases.is_empty() {
                writeln!(f, "test {}", test.path)?;
            } else {
                for (i, case) in test.cases.iter().enumerate() {
                    let args: Vec<_> = case.args.iter().map(|a| format!("{}", a)).collect();
                    writeln!(f, "test {}[{}]({})", test.path, i, args.join(", "))?;
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for TestValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestValue::I32(v) => write!(f, "{}", v),
            TestValue::I64(v) => write!(f, "{}L", v),
            TestValue::F32(v) => write!(f, "{:.1}", v),
            TestValue::F64(v) => write!(f, "{:.1}D", v),
            TestValue::Bool(v) => write!(f, "{}", if *v { "TRUE" } else { "FALSE" }),
        }
    }
}
