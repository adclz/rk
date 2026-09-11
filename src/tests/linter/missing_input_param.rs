use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

// L0303 is a *lint* that fires only on FUNCTION_BLOCK / PROGRAM call sites
// where one or more VAR_INPUT arguments are omitted. The corresponding case
// on FUNCTION/METHOD callees is the hard error E0802 (covered by
// `tests::semantics::func_call`), so this lint deliberately stays silent for
// those to avoid duplicate diagnostics.

#[rstest]
fn fb_missing_one_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK adder
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR inst : adder; END_VAR
    inst(a := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0303] Hint: missing input parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |         b : INT;
        |         ^^^|^^^
        |            `----- 'b' declared here
        |
     11 |     inst(a := 1);
        |     ^^^^^^|^^^^^
        |           `------- call to 'adder' is missing 1 input parameter: 'b'
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn fb_missing_multiple_inputs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK computer
    VAR_INPUT
        x : INT;
        y : INT;
        z : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR inst : computer; END_VAR
    inst(x := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0303] Hint: missing input parameter
        ,-[ file:///test0.st:12:5 ]
        |
      5 |         y : INT;
        |         ^^^|^^^
        |            `----- 'y' declared here
      6 |         z : INT;
        |         ^^^|^^^
        |            `----- 'z' declared here
        |
     12 |     inst(x := 1);
        |     ^^^^^^|^^^^^
        |           `------- call to 'computer' is missing 2 input parameters: 'y', 'z'
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn fb_all_inputs_provided(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK adder
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR inst : adder; END_VAR
    inst(a := 1, b := 2);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @"");
}

// FUNCTION/METHOD callees are E0802 territory - L0303 must stay silent on
// them to avoid duplicating the error.
#[rstest]
fn function_callee_not_linted(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION_BLOCK caller
    add(a := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |     b : INT;
        |     ^^^|^^^
        |        `----- parameter 'b' declared here
        |
     11 |     add(a := 1);
        |     ^|^
        |      `--- call to 'add' is missing 1 required parameter: 'b'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

#[rstest]
fn method_callee_not_linted(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
    METHOD PUBLIC set_values
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
VAR
    fb : MyFb;
END_VAR
    fb.set_values(a := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:15:5 ]
        |
      6 |         b : INT;
        |         ^^^|^^^
        |            `----- parameter 'b' declared here
        |
     15 |     fb.set_values(a := 1);
        |     ^^^^^^|^^^^^^
        |           `-------- call to 'set_values' is missing 1 required parameter: 'b'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

#[rstest]
fn derived_fb_missing_inherited_input(mut with_db: RootDatabase) {
    // The lintable input set is the flattened EXTENDS view: an unwired
    // inherited VAR_INPUT reports exactly like an own one.
    let source = r#"
FUNCTION_BLOCK base_adder
    VAR_INPUT
        a : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived_adder EXTENDS base_adder
    VAR_INPUT
        b : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR inst : derived_adder; END_VAR
    inst(b := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0303] Hint: missing input parameter
        ,-[ file:///test0.st:16:5 ]
        |
      4 |         a : INT;
        |         ^^^|^^^
        |            `----- 'a' declared here
        |
     16 |     inst(b := 1);
        |     ^^^^^^|^^^^^
        |           `------- call to 'derived_adder' is missing 1 input parameter: 'a'
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}
