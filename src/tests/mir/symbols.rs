//! Every function lowers to a symbol of its own: the spellings
//! `mir::lower::naming` promises, and the internal error when two functions
//! would still meet on one.

use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

// What is still left to collide stops lowering rather than lets a call run
// another body. A PROGRAM and a FUNCTION_BLOCK of one name are not refused at
// check yet, and both bodies are `Main$__body__`.
#[rstest]
fn two_functions_under_one_symbol_stop_lowering(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Main VAR x : INT; END_VAR x := x + 1; END_FUNCTION_BLOCK
        PROGRAM Main VAR n : INT; END_VAR n := n + 1; END_PROGRAM
    "#;
    let file = add_source(&mut with_db, source);
    let error = mir::lower::lower_module::lower_module(&with_db, semantic_index(&with_db, file))
        .map(|_| ())
        .expect_err("two bodies under one symbol");
    assert_snapshot!(error, @"two functions lower to the symbol `Main$__body__`");
}
