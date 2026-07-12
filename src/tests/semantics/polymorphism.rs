use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn call_method_on_interface_variable(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork : INT
        VAR_INPUT
            x: INT;
        END_VAR
    END_METHOD
END_INTERFACE

CLASS Worker IMPLEMENTS ITF1
    METHOD OVERRIDE DoWork : INT
        VAR_INPUT
            x: INT;
        END_VAR
        DoWork := x * 2;
    END_METHOD
END_CLASS

PROGRAM A
    VAR
        itf: ITF1;
        result: INT;
    END_VAR

    result := itf.DoWork(x := 5);
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0514] Error: unsupported interface dispatch
        ,-[ file:///test0.st:25:19 ]
        |
      3 |     METHOD DoWork : INT
        |            ^^^|^^
        |               `---- method 'DoWork' is only a prototype, declared in the interface here
        |
     25 |     result := itf.DoWork(x := 5);
        |                   ^^^|^^
        |                      `---- cannot call method 'DoWork' through an interface reference
        |
        | Note: interface methods can not be called directly
    ----'
    ");
}

#[rstest]
fn call_method_on_interface_no_params(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD Reset END_METHOD
END_INTERFACE

CLASS Worker IMPLEMENTS ITF1
    METHOD OVERRIDE Reset END_METHOD
END_CLASS

PROGRAM A
    VAR
        itf: ITF1;
    END_VAR

    itf.Reset();
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0514] Error: unsupported interface dispatch
        ,-[ file:///test0.st:15:9 ]
        |
      3 |     METHOD Reset END_METHOD
        |            ^^|^^
        |              `---- method 'Reset' is only a prototype, declared in the interface here
        |
     15 |     itf.Reset();
        |         ^^|^^
        |           `---- cannot call method 'Reset' through an interface reference
        |
        | Note: interface methods can not be called directly
    ----'
    ");
}

#[rstest]
fn call_nonexistent_method_on_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        itf: ITF1;
    END_VAR

    itf.NonExistent();
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:11:9 ]
        |
      2 | INTERFACE ITF1
        |           ^^|^
        |             `--- INTERFACE 'ITF1' is defined here
        |
     11 |     itf.NonExistent();
        |         ^^^^^|^^^^^
        |              `------- 'ITF1' has no field named 'NonExistent'
    ----'
    ");
}

#[rstest]
fn call_method_wrong_params_on_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork
        VAR_INPUT
            x: INT;
        END_VAR
    END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        itf: ITF1;
    END_VAR

    itf.DoWork(x := TRUE);
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0514] Error: unsupported interface dispatch
        ,-[ file:///test0.st:15:9 ]
        |
      3 |     METHOD DoWork
        |            ^^^|^^
        |               `---- method 'DoWork' is only a prototype, declared in the interface here
        |
     15 |     itf.DoWork(x := TRUE);
        |         ^^^|^^
        |            `---- cannot call method 'DoWork' through an interface reference
        |
        | Note: interface methods can not be called directly
    ----'
    ");
}

#[rstest]
fn interface_extending_interface_method_call(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITFBase
    METHOD BaseMethod : INT END_METHOD
END_INTERFACE

INTERFACE ITFDerived EXTENDS ITFBase
    METHOD DerivedMethod : BOOL END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        itf: ITFDerived;
        x: INT;
        y: BOOL;
    END_VAR

    x := itf.BaseMethod();
    y := itf.DerivedMethod();
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0514] Error: unsupported interface dispatch
        ,-[ file:///test0.st:17:14 ]
        |
      3 |     METHOD BaseMethod : INT END_METHOD
        |            ^^^^^|^^^^
        |                 `------ method 'BaseMethod' is only a prototype, declared in the interface here
        |
     17 |     x := itf.BaseMethod();
        |              ^^^^^|^^^^
        |                   `------ cannot call method 'BaseMethod' through an interface reference
        |
        | Note: interface methods can not be called directly
    ----'
    [E0514] Error: unsupported interface dispatch
        ,-[ file:///test0.st:18:14 ]
        |
      7 |     METHOD DerivedMethod : BOOL END_METHOD
        |            ^^^^^^|^^^^^^
        |                  `-------- method 'DerivedMethod' is only a prototype, declared in the interface here
        |
     18 |     y := itf.DerivedMethod();
        |              ^^^^^^|^^^^^^
        |                    `-------- cannot call method 'DerivedMethod' through an interface reference
        |
        | Note: interface methods can not be called directly
    ----'
    ");
}

#[rstest]
fn interface_variable_as_direct_type_error(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        x: INT;
    END_VAR

    x := ITF1;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:11:10 ]
        |
     11 |     x := ITF1;
        |          ^^|^
        |            `--- cannot use direct type 'ITF1' here
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:10 ]
        |
      8 |         x: INT;
        |         |
        |         `-- type is declared by variable 'x' here
        |
     11 |     x := ITF1;
        |          ^^|^
        |            `--- expected 'INT', got 'ITF1'
    ----'
    ");
}

#[rstest]
fn assign_class_instance_to_interface_variable(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

CLASS Worker IMPLEMENTS ITF1
    METHOD OVERRIDE DoWork END_METHOD
END_CLASS

PROGRAM A
    VAR
        w: Worker;
        itf: ITF1;
    END_VAR

    itf := w;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_fb_instance_to_interface_variable(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK WorkerFB IMPLEMENTS ITF1
    METHOD OVERRIDE DoWork END_METHOD
END_FUNCTION_BLOCK

PROGRAM A
    VAR
        w: WorkerFB;
        itf: ITF1;
    END_VAR

    itf := w;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_non_implementing_class_to_interface_error(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

CLASS Worker
    METHOD DoWork END_METHOD
END_CLASS

PROGRAM A
    VAR
        w: Worker;
        itf: ITF1;
    END_VAR

    itf := w;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:16:12 ]
        |
      2 | INTERFACE ITF1
        |           ^^|^
        |             `--- INTERFACE 'ITF1' is defined here
        |
     16 |     itf := w;
        |            |
        |            `-- expected 'ITF1', got 'Worker'
    ----'
    ");
}

#[rstest]
fn assign_derived_interface_to_base_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITFBase
    METHOD BaseMethod END_METHOD
END_INTERFACE

INTERFACE ITFDerived EXTENDS ITFBase
    METHOD DerivedMethod END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        derived: ITFDerived;
        base: ITFBase;
    END_VAR

    base := derived;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_unrelated_interface_error(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

INTERFACE ITF2
    METHOD OtherWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        itf1: ITF1;
        itf2: ITF2;
    END_VAR

    itf1 := itf2;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:16:13 ]
        |
      2 | INTERFACE ITF1
        |           ^^|^
        |             `--- INTERFACE 'ITF1' is defined here
        |
     16 |     itf1 := itf2;
        |             ^^|^
        |               `--- expected 'ITF1', got 'ITF2'
    ----'
    ");
}

#[rstest]
fn assign_class_implementing_transitive_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITFBase
    METHOD BaseMethod END_METHOD
END_INTERFACE

INTERFACE ITFDerived EXTENDS ITFBase
    METHOD DerivedMethod END_METHOD
END_INTERFACE

CLASS Worker IMPLEMENTS ITFDerived
    METHOD OVERRIDE DerivedMethod END_METHOD
END_CLASS

PROGRAM A
    VAR
        w: Worker;
        base: ITFBase;
    END_VAR

    base := w;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
