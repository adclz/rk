// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test NAME`: which tests a name selects.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::rstest;

/// A test's name is an identifier, so the filter matches it in any case:
/// `rk test T_ADD` runs `t_add`. It found nothing.
#[rstest]
fn a_test_name_is_matched_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION t_add
        END_FUNCTION

        {test}
        FUNCTION t_sub
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let names: Vec<String> = rk::test_host::run_each(&wasm, Some("T_ADD"), None, |_| {})
        .expect("the tests run")
        .into_iter()
        .map(|record| record.name)
        .collect();
    assert_eq!(names, ["t_add"]);
}
