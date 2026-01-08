use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
#[case("REAL")]
#[case("LREAL")]
fn valid_real_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := 0.0;
        test2: {typ} := 10.2;
        test3: {typ} := -58.50;
        test4: {typ} := -50.005555;
        test5: {typ} := 1.0E3;
        test6: {typ} := 1.0e-3;
        test7: {typ} := REAL#10.5;
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @r""); }
}
