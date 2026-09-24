//! Durations as a person types them: `30s`, `500ms`, `2m`, a bare number
//! as seconds, and IEC's own `T#30s`.

use std::time::Duration;

use crate::error::{CliError, CliResult};

/// Parse a duration, or say what was wrong with it.
pub fn parse(text: &str) -> CliResult<Duration> {
    let t = text.trim();
    // `T#30s` is how a duration is written in ST; accept it rather than
    // refusing the one spelling the rest of the toolchain uses.
    let t = t
        .strip_prefix("T#")
        .or_else(|| t.strip_prefix("t#"))
        .unwrap_or(t);

    let (value, unit) = match t.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (&t[..i], &t[i..]),
        None => (t, "s"),
    };
    let value: f64 = value.parse().map_err(|_| invalid(text))?;
    if value < 0.0 {
        return Err(invalid(text));
    }
    let scale = match unit {
        "ms" => 1e-3,
        "s" => 1.0,
        "m" | "min" => 60.0,
        "h" => 3600.0,
        _ => return Err(invalid(text)),
    };
    Ok(Duration::from_secs_f64(value * scale))
}

fn invalid(text: &str) -> CliError {
    CliError::msg(format!(
        "`{text}` is not a duration; write it like 30s, 500ms, 2m or T#30s"
    ))
}
