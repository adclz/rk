use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn function_as_var_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: fn;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0215] Error: invalid type
       ,-[ file:///test0.st:8:15 ]
       |
     8 |         test: fn;
       |               ^|
       |                `-- 'fn' is a function and cannot be used as a variable or data type
    ---'
    ");
}

// IEC 61131-3: RETAIN/NON_RETAIN may qualify VAR_INPUT, VAR_OUTPUT, and VAR of
// function blocks and programs — but never VAR_IN_OUT (a by-reference binding
// to the caller's storage has no state of its own to retain). The grammar
// enforces this structurally: `in_out_decls` has no retain field.
#[rstest]
fn invalid_var_in_out_retain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_IN_OUT RETAIN
        io: INT;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:3:16 ]
       |
     3 |     VAR_IN_OUT RETAIN
       |                ^^^|^^
       |                   `---- Unexpected token(s): 'RETAIN'
    ---'
    ");
}

// The legal placements from the same clause: RETAIN on VAR_INPUT, VAR_OUTPUT,
// and VAR of an FB and a PROGRAM all parse and type-check clean.
#[rstest]
fn valid_retain_qualifier_placements(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_INPUT RETAIN
        a: INT;
    END_VAR
    VAR_OUTPUT NON_RETAIN
        b: INT;
    END_VAR
    VAR RETAIN
        c: INT;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM prog1
    VAR RETAIN
        d: INT;
    END_VAR
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
