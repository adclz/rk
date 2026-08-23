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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_units_a_person_writes() {
        assert_eq!(parse("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse("1h").unwrap(), Duration::from_secs(3600));
        // A bare number is seconds: no unit means the obvious one.
        assert_eq!(parse("45").unwrap(), Duration::from_secs(45));
        // The ST spelling, because that is what the source files use.
        assert_eq!(parse("T#30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse("1.5s").unwrap(), Duration::from_millis(1500));
        assert_eq!(parse(" 30s ").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn nonsense_is_refused_with_the_shape_that_works() {
        for bad in ["", "abc", "30x", "-5s", "s"] {
            let CliError::Message(msg) = parse(bad).expect_err(bad) else {
                panic!("`{bad}` must be refused with a message");
            };
            assert!(
                msg.contains("30s"),
                "the refusal must show a working example: {msg}"
            );
        }
    }
}
