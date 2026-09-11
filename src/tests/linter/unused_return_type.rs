use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn function_call_discards_return_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION test : INT
VAR
    x : INT;
END_VAR
    add(a := 1, b := 2);
    test := x;
END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"
    [L0302] Hint: unused return value
        ,-[ file:///test0.st:14:5 ]
        |
      2 | FUNCTION add : INT
        |          ^|^
        |           `--- FUNCTION 'add' is defined here, with return type 'INT'
        |
     14 |     add(a := 1, b := 2);
        |     ^^^^^^^^^|^^^^^^^^^
        |              `----------- unused return value of 'add'
        |
        | Note: lint rule: unused-return-type
    ----'
    ");
}

#[rstest]
fn return_value_assigned(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x := add(a := 1, b := 2);
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"");
}

#[rstest]
fn no_return_type_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK do_something
        VAR_INPUT
            a : INT;
        END_VAR
        VAR
            local : INT;
        END_VAR
            local := a + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            fb : do_something;
        END_VAR
            fb(a := 1);
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"");
}

#[rstest]
fn function_block_call_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            c : Counter;
        END_VAR
            c();
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"");
}

#[rstest]
fn method_call_discards_return_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
    METHOD PUBLIC get_value : INT
        get_value := 42;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR
    fb : MyFb;
END_VAR
    fb.get_value();
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"
    [L0302] Hint: unused return value
        ,-[ file:///test0.st:12:5 ]
        |
      3 |     METHOD PUBLIC get_value : INT
        |                   ^^^^|^^^^
        |                       `------ METHOD 'get_value' is defined here, with return type 'INT'
        |
     12 |     fb.get_value();
        |     ^^^^^^^|^^^^^^
        |            `-------- unused return value of 'get_value'
        |
        | Note: lint rule: unused-return-type
    ----'
    ");
}

#[rstest]
fn method_return_value_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
    METHOD PUBLIC get_value : INT
        get_value := 42;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR
    fb : MyFb;
END_VAR
    test := fb.get_value();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @"");
}

#[rstest]
fn method_no_return_type_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
    METHOD PUBLIC set_value
    VAR_INPUT
        _v : INT;
    END_VAR
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR
    fb : MyFb;
END_VAR
    fb.set_value(_v := 10);
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @"");
}

#[rstest]
fn fb_body_discards_function_return_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION compute : INT
VAR_INPUT
    x : INT;
END_VAR
    compute := x * 2;
END_FUNCTION

FUNCTION_BLOCK Worker
VAR
    inst_x : INT;
END_VAR
    compute(x := inst_x);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-return-type"), @r"
    [L0302] Hint: unused return value
        ,-[ file:///test0.st:13:5 ]
        |
      2 | FUNCTION compute : INT
        |          ^^^|^^^
        |             `----- FUNCTION 'compute' is defined here, with return type 'INT'
        |
     13 |     compute(x := inst_x);
        |     ^^^^^^^^^^|^^^^^^^^^
        |               `----------- unused return value of 'compute'
        |
        | Note: lint rule: unused-return-type
    ----'
    ");
}
