// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test NAME`: which tests a name selects.

use crate::tests::codegen::{compile_tests, with_db};
use rstest::rstest;

/// A test's name is an identifier, so the filter matches it in any case:
/// `rk test T_SUB` runs `t_sub`. It found nothing. On the component, the
/// test keeps its place among all of them, which is what `run-test` takes:
/// `t_sub` is the second, and fails with its own message.
#[rstest]
fn a_test_name_is_matched_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION t_add
        END_FUNCTION

        {test}
        FUNCTION t_sub
            __RAISE('t_sub ran');
        END_FUNCTION
    "#;
    let build = compile_tests(&mut with_db, source);
    let records: Vec<(String, Option<String>)> =
        rk::test_host::run_each(&build.component, Some("T_SUB"), None, |_| {})
            .expect("the tests run")
            .into_iter()
            .map(|record| (record.name, record.reason))
            .collect();
    assert_eq!(
        records,
        [("t_sub".to_string(), Some("t_sub ran".to_string()))]
    );
}
