use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_assign_attempt_ref_to_ref(mut with_db: RootDatabase) {
    let source = r#"
CLASS ClBase
END_CLASS

CLASS ClDerived EXTENDS ClBase
END_CLASS

PROGRAM A
    VAR
        instBase: ClBase;
        instDerived: ClDerived;
        rinstBase: REF_TO ClBase;
        rinstDerived: REF_TO ClDerived;
    END_VAR

    rinstBase := REF(instBase);
    rinstDerived ?= rinstBase;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_assign_attempt_interface_to_ref(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
END_INTERFACE

CLASS ClBase IMPLEMENTS ITF1
END_CLASS

PROGRAM A
    VAR
        interf: ITF1;
        rinstBase: REF_TO ClBase;
    END_VAR

    rinstBase ?= interf;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_assign_attempt_lhs_not_ref(mut with_db: RootDatabase) {
    let source = r#"
CLASS ClBase
END_CLASS

PROGRAM A
    VAR
        instBase: ClBase;
        rinstBase: REF_TO ClBase;
    END_VAR

    instBase ?= rinstBase;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0319] Error: invalid assignment attempt
        ,-[ file:///test0.st:11:5 ]
        |
      2 | CLASS ClBase
        |       ^^^|^^
        |          `---- CLASS 'ClBase' is defined here
        |
     11 |     instBase ?= rinstBase;
        |     ^^^^|^^^
        |         `----- assignment attempt '?=' requires a REF_TO variable, got 'ClBase'
    ----'
    ");
}

#[rstest]
fn invalid_assign_attempt_lhs_elementary(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM A
    VAR
        x: INT;
        y: INT;
    END_VAR

    x ?= y;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0319] Error: invalid assignment attempt
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     x ?= y;
       |     |
       |     `-- assignment attempt '?=' requires a REF_TO variable, got 'INT'
    ---'
    ");
}

#[rstest]
fn invalid_assign_attempt_rhs_elementary(mut with_db: RootDatabase) {
    let source = r#"
CLASS ClBase
END_CLASS

PROGRAM A
    VAR
        rinstBase: REF_TO ClBase;
        x: INT;
    END_VAR

    rinstBase ?= x;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0320] Error: invalid assignment attempt
        ,-[ file:///test0.st:11:18 ]
        |
     11 |     rinstBase ?= x;
        |                  |
        |                  `-- assignment attempt '?=' requires a REF_TO or interface on the right-hand side, got 'INT'
    ----'
    ");
}

#[rstest]
fn invalid_assign_attempt_rhs_class_instance(mut with_db: RootDatabase) {
    let source = r#"
CLASS ClBase
END_CLASS

PROGRAM A
    VAR
        rinstBase: REF_TO ClBase;
        inst: ClBase;
    END_VAR

    rinstBase ?= inst;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0320] Error: invalid assignment attempt
        ,-[ file:///test0.st:11:18 ]
        |
      2 | CLASS ClBase
        |       ^^^|^^
        |          `---- CLASS 'ClBase' is defined here
        |
     11 |     rinstBase ?= inst;
        |                  ^^|^
        |                    `--- assignment attempt '?=' requires a REF_TO or interface on the right-hand side, got 'ClBase'
    ----'
    ");
}

// Standard example (Table 52): full assignment attempt scenario
#[rstest]
fn valid_standard_example(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
END_INTERFACE

INTERFACE ITF2
END_INTERFACE

CLASS ClBase IMPLEMENTS ITF1, ITF2
END_CLASS

CLASS ClDerived EXTENDS ClBase
END_CLASS

PROGRAM A
    VAR
        instBase: ClBase;
        rinstBase1: REF_TO ClBase;
        rinstDerived1: REF_TO ClDerived;
        rinstDerived3: REF_TO ClDerived;
        rinstDerived4: REF_TO ClDerived;
        interf1: ITF1;
        interf2: ITF2;
    END_VAR

    rinstBase1 := REF(instBase);
    rinstDerived1 ?= rinstBase1;
    rinstDerived3 ?= interf1;
    rinstDerived4 ?= interf2;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
