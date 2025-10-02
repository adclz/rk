use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_tod_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := tod#00:00:00.0;
        test2: TOD := TOD#12:20:50.552;
        test3: TIME_OF_DAY := TIME_OF_DAY#12:30:45.123
    END_VAR
END_FUNCTION_BLOCK"#.to_string();

    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

#[rstest]
fn valid_l_tod_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTOD := ltod#00:00:00.0;
        test2: LTOD := LTOD#12:20:50.552;
        test3: LTIME_OF_DAY := LTIME_OF_DAY#12:30:45.123
    END_VAR
END_FUNCTION_BLOCK"#.to_string();

    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}
