//! Terminal output for the CLI: the single place that reaches for color
//! and picks a stream. Problems ([`error`]/[`warn`]/[`failure`]) go to
//! stderr; results ([`success`]/[`detail`]) to stdout, or to stderr via
//! [`success_err`] when stdout is a transport.

use std::fmt::Display;
use std::sync::OnceLock;

use yansi::Paint;

use crate::cli::OutputFormat;

static FORMAT: OnceLock<OutputFormat> = OnceLock::new();

/// Decide color for the whole process, once: on only when the format is
/// `full`, `NO_COLOR` is unset (https://no-color.org) and stderr is a
/// terminal. Every paint and every ariadne report follows this switch.
pub fn init_output(format: OutputFormat) {
    use std::io::IsTerminal;
    let _ = FORMAT.set(format);
    let color = format == OutputFormat::Full
        && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
        && std::io::stderr().is_terminal();
    if !color {
        yansi::disable();
    }
}

/// The format every message follows: `full` until [`init_output`] ran, which
/// is the TUI path and the tests.
pub fn format() -> OutputFormat {
    FORMAT.get().copied().unwrap_or(OutputFormat::Full)
}

/// One message as a `json-lines` record, on the stream the prose would
/// take.
fn record(level: &str, label: Option<&str>, msg: &str) -> String {
    let mut value = serde_json::json!({ "type": "message", "level": level, "text": msg });
    if let Some(label) = label {
        value["label"] = serde_json::Value::String(label.trim_end_matches(':').to_string());
    }
    value.to_string()
}

fn machine() -> bool {
    format() == OutputFormat::JsonLines
}

/// `error: <msg>` on stderr.
pub fn error(msg: impl Display) {
    if machine() {
        return eprintln!("{}", record("error", None, &msg.to_string()));
    }
    eprintln!("{}{msg}", "error: ".bold().red());
}

/// `warning: <msg>` on stderr.
pub fn warn(msg: impl Display) {
    if machine() {
        return eprintln!("{}", record("warning", None, &msg.to_string()));
    }
    eprintln!("{}{msg}", "warning: ".bold().yellow());
}

/// A red-labelled failure line on stderr: `<label> <msg>`. For a final failure
/// summary that isn't a single `error:` (e.g. `compilation failed: …`).
pub fn failure(label: &str, msg: impl Display) {
    if machine() {
        return eprintln!("{}", record("failure", Some(label), &msg.to_string()));
    }
    eprintln!("{} {msg}", label.bold().red());
}

/// A green-labelled result line on stdout: `<label> <msg>` (e.g. `compiled:`).
pub fn success(label: &str, msg: impl Display) {
    if machine() {
        return println!("{}", record("success", Some(label), &msg.to_string()));
    }
    println!("{} {msg}", label.bold().bright_green());
}

/// [`success`] on stderr — for a command whose stdout is a transport.
pub fn success_err(label: &str, msg: impl Display) {
    if machine() {
        return eprintln!("{}", record("success", Some(label), &msg.to_string()));
    }
    eprintln!("{} {msg}", label.bold().bright_green());
}

/// A dim progress line on stdout (e.g. "watching for changes…").
pub fn detail(msg: impl Display) {
    if machine() {
        return println!("{}", record("detail", None, &msg.to_string()));
    }
    println!("{}", msg.dim());
}
