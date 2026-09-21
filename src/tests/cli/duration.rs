use std::time::Duration;

use rk::duration::parse;
use rk::error::CliError;

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
