//! Test manifest: metadata about test functions, embedded in the compiled
//! module as the [`TEST_MANIFEST_SECTION`] custom section, so it can never
//! drift from the wasm it describes.

use serde::{Deserialize, Serialize};

/// Name of the custom wasm section that carries the MessagePack-encoded
/// [`TestManifest`] inside the compiled component.
pub const TEST_MANIFEST_SECTION: &str = "test-manifest";

/// The complete test manifest for a module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestManifest {
    pub tests: Vec<TestEntry>,
}

/// A single test function entry. MessagePack encodes positionally: new
/// fields go at the tail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestEntry {
    /// Fully qualified path (e.g., "Std.Math.Test.test_abs").
    pub path: String,

    /// WASM export name for the test.
    pub export: String,

    /// Workspace-relative source file declaring the test (empty if unknown).
    pub file: String,

    /// 1-based line of the test declaration (0 if unknown).
    pub line: u32,
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
            writeln!(f, "test {}", test.path)?;
        }
        Ok(())
    }
}
