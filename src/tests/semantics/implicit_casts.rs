use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Implicit casts according to IEC 61131-3 standard
///
/// See 6.6.1.6 Data type conversion

#[rstest]
#[case("TRUE")]
#[case("FALSE")]
#[case("BOOL#TRUE")]
#[case("BOOL#FALSE")]
fn bool_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: BYTE := {value};
        test2: WORD := {value};
        test3: DWORD := {value};
        test4: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("BYTE#0")]
fn byte_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: BYTE := {value};
        test2: WORD := {value};
        test3: DWORD := {value};
        test4: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("WORD#0")]
fn word_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: WORD := {value};
        test2: DWORD := {value};
        test3: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("DWORD#0")]
fn dword_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DWORD := {value};
        test2: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("LWORD#0")]
fn lword_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("SINT#0")]
fn sint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: SINT := {value};
        test2: INT := {value};
        test3: DINT := {value};
        test4: LINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("INT#0")]
fn int_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := {value};
        test2: DINT := {value};
        test3: LINT := {value};
        test4: REAL := {value};
        test5: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("DINT#0")]
fn dint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: DINT := {value};
        test3: LINT := {value};
        // test4: REAL := {value}; // no real (see table in standard)
        test5: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0.0")]
#[case("REAL#0.0")]
fn real_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: REAL := {value};
        test3: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("USINT#0")]
fn usint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: USINT := {value};
        test2: UINT := {value};
        test3: UDINT := {value};
        test4: ULINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("UINT#0")]
fn uint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: UINT := {value};
        test3: UDINT := {value};
        test4: ULINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("UDINT#0")]
fn udint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test3: UDINT := {value};
        test4: ULINT := {value};
        // test5: REAL := {value}; (same as sint)
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("TIME#0s")]
fn time_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
        test2: LTIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("DT#0d")]
fn dt_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DT := {value};
        test2: LDT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("TOD#0h")]
fn tod_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := {value};
        test2: LTOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}
