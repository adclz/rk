// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn variable_and_method_of_one_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Inner
VAR_OUTPUT n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
VAR
    step : Inner;
END_VAR
    METHOD PUBLIC STEP
        step();
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "variable-method-name"), @r"
    [L0120] Warning: variable and method with the same name
        ,-[ file:///test0.st:11:19 ]
        |
      9 |     step : Inner;
        |     ^^|^
        |       `--- variable 'step' is declared here
        |
     11 |     METHOD PUBLIC STEP
        |                   ^^|^
        |                     `--- method 'STEP' has the name of the variable 'step'
        |
        | Note 1: inside the block, `step` and `THIS.step` are the variable
        |
        | Note 2: lint rule: variable-method-name
    ----'
    ");
}

#[rstest]
fn variable_named_like_an_inherited_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
    METHOD PUBLIC Run : INT
        Run := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
VAR
    run : INT;
END_VAR
    run := 2;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "variable-method-name"), @r"
    [L0120] Warning: variable and method with the same name
        ,-[ file:///test0.st:10:5 ]
        |
      3 |     METHOD PUBLIC Run : INT
        |                   ^|^
        |                    `--- method 'Run' is declared here
        |
     10 |     run : INT;
        |     ^|^
        |      `--- variable 'run' has the name of the method 'Run' it inherits
        |
        | Note 1: inside the block, `run` and `THIS.run` are the variable
        |
        | Note 2: lint rule: variable-method-name
    ----'
    ");
}

#[rstest]
fn method_named_like_an_inherited_variable(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
VAR
    speed : INT;
END_VAR
END_CLASS

CLASS Derived EXTENDS Base
    METHOD PUBLIC Speed : INT
        Speed := 2;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "variable-method-name"), @r"
    [L0120] Warning: variable and method with the same name
       ,-[ file:///test0.st:9:19 ]
       |
     4 |     speed : INT;
       |     ^^|^^
       |       `---- variable 'speed' is declared here
       |
     9 |     METHOD PUBLIC Speed : INT
       |                   ^^|^^
       |                     `---- method 'Speed' has the name of the variable 'speed' it inherits
       |
       | Note 1: inside the block, `speed` and `THIS.speed` are the variable
       |
       | Note 2: lint rule: variable-method-name
    ---'
    ");
}

/// A method's own variable is L0116's case, not this one.
#[rstest]
fn a_method_variable_is_not_a_member(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR
    x : INT;
END_VAR
    METHOD PUBLIC Get : INT
    VAR x : INT; END_VAR
        x := 3;
        Get := x;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "variable-method-name"), @"");
}
