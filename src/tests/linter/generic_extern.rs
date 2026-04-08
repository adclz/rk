use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn extern_with_any_real_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SQRT : ANY_REAL
VAR_INPUT
    IN : INTO(SQRT);
END_VAR
    {extern 'math' 'sqrt' (params IN) (result SQRT)}
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "generic-extern"), @r"
    [L0120] Info: generic extern function
       ,-[ file:///test0.st:6:35 ]
       |
     4 |     IN : INTO(SQRT);
       |     ^|
       |      `-- 'IN' is declared as INTO
       |
     6 |     {extern 'math' 'sqrt' (params IN) (result SQRT)}
       |                                   ^|
       |                                    `-- extern parameter 'IN' has generic type INTO: the host must provide an implementation for each concrete type variant
       |
       | Note: lint rule: generic-extern
    ---'
    [L0120] Info: generic extern function
       ,-[ file:///test0.st:6:47 ]
       |
     2 | FUNCTION SQRT : ANY_REAL
       |                 ^^^^|^^^
       |                     `----- return type is ANY_REAL
       |
     6 |     {extern 'math' 'sqrt' (params IN) (result SQRT)}
       |                                               ^^|^
       |                                                 `--- extern result 'SQRT' has generic return type ANY_REAL: the host must provide an implementation for each concrete type variant
       |
       | Note: lint rule: generic-extern
    ---'
    ");
}

#[rstest]
fn extern_without_generics(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION now : ULINT
    {extern 'wasi:clocks/monotonic-clock@0.2.6' 'now' (result now)}
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "generic-extern"), @r"");
}

#[rstest]
fn extern_with_any_num(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT
    IN : INTO(ABS);
END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "generic-extern"), @r"
    [L0120] Info: generic extern function
       ,-[ file:///test0.st:6:34 ]
       |
     4 |     IN : INTO(ABS);
       |     ^|
       |      `-- 'IN' is declared as INTO
       |
     6 |     {extern 'math' 'abs' (params IN) (result ABS)}
       |                                  ^|
       |                                   `-- extern parameter 'IN' has generic type INTO: the host must provide an implementation for each concrete type variant
       |
       | Note: lint rule: generic-extern
    ---'
    [L0120] Info: generic extern function
       ,-[ file:///test0.st:6:46 ]
       |
     2 | FUNCTION ABS : ANY_NUM
       |                ^^^|^^^
       |                   `----- return type is ANY_NUM
       |
     6 |     {extern 'math' 'abs' (params IN) (result ABS)}
       |                                              ^|^
       |                                               `--- extern result 'ABS' has generic return type ANY_NUM: the host must provide an implementation for each concrete type variant
       |
       | Note: lint rule: generic-extern
    ---'
    ");
}
