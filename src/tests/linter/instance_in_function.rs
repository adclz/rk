use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

const POUS: &str = r#"
FUNCTION_BLOCK Motor
VAR n : INT; END_VAR
END_FUNCTION_BLOCK

CLASS Sensor
VAR n : INT; END_VAR
END_CLASS
"#;

/// A FUNCTION or METHOD is stateless: an instance it holds starts over at
/// every call, so a timer in one never expires, and an instance it returns is
/// made afresh by each call.
#[rstest]
#[case::var(
    "FUNCTION f : INT VAR i : Motor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a FUNCTION_BLOCK instance, which starts over at every call of the FUNCTION"
)]
#[case::var_class(
    "FUNCTION f : INT VAR i : Sensor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a CLASS instance, which starts over at every call of the FUNCTION"
)]
#[case::temp(
    "FUNCTION f : INT VAR_TEMP i : Motor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a FUNCTION_BLOCK instance"
)]
#[case::input(
    "FUNCTION f : INT VAR_INPUT i : Motor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a FUNCTION_BLOCK instance"
)]
#[case::output(
    "FUNCTION f : INT VAR_OUTPUT i : Sensor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a CLASS instance"
)]
#[case::array(
    "FUNCTION f : INT VAR i : ARRAY[0..1] OF Motor; END_VAR f := 1; END_FUNCTION",
    "'i' holds a FUNCTION_BLOCK instance"
)]
#[case::method_var(
    "FUNCTION_BLOCK H METHOD PUBLIC M : INT VAR i : Motor; END_VAR M := 1; END_METHOD END_FUNCTION_BLOCK",
    "'i' holds a FUNCTION_BLOCK instance, which starts over at every call of the METHOD"
)]
#[case::returned(
    "FUNCTION f : Motor END_FUNCTION",
    "the FUNCTION returns a FUNCTION_BLOCK instance, made afresh by each call"
)]
#[case::returned_by_a_method(
    "FUNCTION_BLOCK H METHOD PUBLIC M : Sensor END_METHOD END_FUNCTION_BLOCK",
    "the METHOD returns a CLASS instance, made afresh by each call"
)]
fn an_instance_in_a_function_is_warned(
    mut with_db: RootDatabase,
    #[case] pou: &str,
    #[case] message: &str,
) {
    let source = format!("{POUS}\n{pou}\n");
    let rendered = test_single_lint(&mut with_db, &[&source], "instance-in-function");
    assert_eq!(
        rendered.matches("[L0119]").count(),
        1,
        "one warning, got:\n{rendered}"
    );
    assert!(
        rendered.contains(message),
        "expected `{message}`, got:\n{rendered}"
    );
}

/// Where the instance lives elsewhere, nothing starts over: a VAR_IN_OUT is
/// the caller's, a VAR_EXTERNAL the global's, and one a FUNCTION_BLOCK holds
/// is its own state. A `{test}` FUNCTION is an entry the runner calls once.
#[rstest]
fn an_instance_that_lives_elsewhere_is_not_warned(mut with_db: RootDatabase) {
    let source = format!(
        "{POUS}{}",
        r#"
FUNCTION kick : INT
VAR_IN_OUT m : Motor; END_VAR
VAR_EXTERNAL g : Motor; END_VAR
    kick := 1;
END_FUNCTION

FUNCTION_BLOCK Holder
VAR m : Motor; s : Sensor; END_VAR
END_FUNCTION_BLOCK

{test}
FUNCTION test_motor
VAR m : Motor; END_VAR
END_FUNCTION

CONFIGURATION Cfg
VAR_GLOBAL g : Motor; END_VAR
END_CONFIGURATION
"#
    );
    assert_snapshot!(test_single_lint(&mut with_db, &[&source], "instance-in-function"), @r"");
}

/// How the warning reads, for a variable and for a result.
#[rstest]
fn instance_in_function_rendering(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK TON_ms
VAR n : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION debounce : TON_ms
VAR t : TON_ms; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "instance-in-function"), @r"
    [L0119] Warning: instance in a stateless POU
       ,-[ file:///test0.st:7:5 ]
       |
     7 | VAR t : TON_ms; END_VAR
       |     |
       |     `-- 't' holds a FUNCTION_BLOCK instance, which starts over at every call of the FUNCTION
       |
       | Note 1: FUNCTIONs and METHODs are stateless
       |
       | Note 2: lint rule: instance-in-function
    ---'
    [L0119] Warning: instance in a stateless POU
       ,-[ file:///test0.st:6:21 ]
       |
     6 | FUNCTION debounce : TON_ms
       |                     ^^^|^^
       |                        `---- the FUNCTION returns a FUNCTION_BLOCK instance, made afresh by each call
       |
       | Note 1: FUNCTIONs and METHODs are stateless
       |
       | Note 2: lint rule: instance-in-function
    ---'
    ");
}
