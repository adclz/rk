use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_time_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := time#0s;
        test2: TIME := t#0m0s;
        test3: TIME := T#0h0m0s;
        test4: TIME := TIME#0s;
    END_VAR
END_FUNCTION_BLOCK"#.to_string();

    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

#[rstest]
fn valid_l_time_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTIME := ltime#0s;
        test2: LTIME := lt#0m0s;
        test3: LTIME := LT#0h0m0s;
        test4: LTIME := LTIME#0s;
    END_VAR
END_FUNCTION_BLOCK"#.to_string();

    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}
