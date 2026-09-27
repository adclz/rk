//! The compiler's fuzzing oracles, and the corpus they start from.
//!
//! A fuzz target only finds what its oracle can see, and "did not panic" sees
//! little. Each module here states a property the compiler promises and
//! checks it:
//!
//! - [`pipeline`]: a source `rk check` accepts must lower, and emit a module
//!   that validates against the features rk claims, carries custom sections
//!   that decode and agree with the module, instantiates, and runs.
//! - [`incremental`]: the editor's path (an edit applied to a live
//!   database, reparsed incrementally) must end where a fresh build of the
//!   same text does.
//! - [`format`]: the formatter's output parses, is a fixed point, and means
//!   what its input meant.
//! - [`semantics`]: a program computes the values its header says, which
//!   [`generate`] works out for the programs it writes.
//!
//! The libFuzzer targets in `fuzz_targets/` panic on a [`Finding`]; the
//! `repro` binary runs the same checks on files and names the one that
//! failed, and `tests/regressions.rs` keeps every past finding fixed.

pub mod corpus;
pub mod format;
pub mod generate;
pub mod incremental;
pub mod pipeline;
pub mod semantics;
mod session;
mod wasm;

pub use corpus::{create_seed_corpus, load_corpus};

/// A broken promise: the compiler did something no input can excuse.
///
/// Anything an input CAN legitimately cause (a diagnostic, a trap the
/// program asked for, an exhausted fuel budget) is not a finding, so a
/// finding is a bug by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Which property broke, stable across inputs: crash reports are
    /// grouped by it.
    pub oracle: &'static str,
    pub detail: String,
}

impl Finding {
    pub(crate) fn new(oracle: &'static str, detail: impl Into<String>) -> Self {
        Self {
            oracle,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.oracle, self.detail)
    }
}

impl std::error::Error for Finding {}

/// Every oracle over one source, in the order a user meets them: compile
/// it, format it, edit it. The verdict says how far the compiler got.
pub fn check_all(source: &str) -> Result<pipeline::Verdict, Finding> {
    let verdict = pipeline::check(source)?;
    semantics::check(source)?;
    format::check(source)?;
    incremental::check(source)?;
    Ok(verdict)
}

/// The input as text, or `None` when it is not UTF-8 or too large to be
/// worth an iteration. The editor never hands the compiler anything else.
pub fn as_source(case: &[u8]) -> Option<&str> {
    if case.is_empty() || case.len() > 64 * 1024 {
        return None;
    }
    std::str::from_utf8(case).ok()
}
