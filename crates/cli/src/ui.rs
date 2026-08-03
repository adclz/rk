//! Centralized terminal output for the CLI.
//!
//! The single place the CLI reaches for color and picks a stream, so the rest of
//! the code never touches `yansi` directly. Convention: problems
//! ([`error`]/[`warn`]/[`failure`]) go to **stderr**; results and progress
//! ([`success`]/[`detail`]) go to **stdout** — except the debugger, whose stdout is
//! the debugger transport, so it uses [`success_err`] to keep human output on stderr.

use std::fmt::Display;

use yansi::Paint;

/// Decide color for the whole process, once: on only when the format is
/// `full`, `NO_COLOR` is unset (https://no-color.org) and stderr is a
/// terminal. Every paint and every ariadne report follows this switch.
pub fn init_output(format: crate::cli::OutputFormat) {
    use std::io::IsTerminal;
    let color = format == crate::cli::OutputFormat::Full
        && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
        && std::io::stderr().is_terminal();
    if !color {
        yansi::disable();
    }
}

/// `error: <msg>` on stderr.
pub fn error(msg: impl Display) {
    eprintln!("{}{msg}", "error: ".bold().red());
}

/// `warning: <msg>` on stderr.
pub fn warn(msg: impl Display) {
    eprintln!("{}{msg}", "warning: ".bold().yellow());
}

/// A red-labelled failure line on stderr: `<label> <msg>`. For a final failure
/// summary that isn't a single `error:` (e.g. `compilation failed: …`).
pub fn failure(label: &str, msg: impl Display) {
    eprintln!("{} {msg}", label.bold().red());
}

/// A green-labelled result line on stdout: `<label> <msg>` (e.g. `compiled:`).
pub fn success(label: &str, msg: impl Display) {
    println!("{} {msg}", label.bold().bright_green());
}

/// [`success`] on stderr — for the debugger, whose stdout is the debugger transport.
pub fn success_err(label: &str, msg: impl Display) {
    eprintln!("{} {msg}", label.bold().bright_green());
}

/// A dim progress line on stdout (e.g. "watching for changes…").
pub fn detail(msg: impl Display) {
    println!("{}", msg.dim());
}
