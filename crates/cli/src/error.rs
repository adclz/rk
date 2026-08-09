//! CLI error type: [`CliError::Message`] is a hard error (`error: <msg>`,
//! exit 1); [`CliError::Failed`] means the command found problems and
//! already printed its own summary, so `main` exits non-zero silently.

use std::fmt::Display;

/// Result returned by every CLI command. `Ok` ⇒ exit 0.
pub type CliResult<T = ()> = Result<T, CliError>;

/// A CLI command failure.
#[derive(Debug)]
pub enum CliError {
    /// A hard error: `main` prints `error: <msg>` and exits non-zero.
    Message(String),
    /// The command ran but found problems it already reported; `main` exits
    /// non-zero with no extra output.
    Failed,
}

impl CliError {
    /// A hard error from any displayable value (an `anyhow::Error`, a `String`,
    /// an `io::Error`, …).
    pub fn msg(m: impl Display) -> Self {
        CliError::Message(m.to_string())
    }
}
