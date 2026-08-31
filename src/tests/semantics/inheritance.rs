use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn missing_override(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // missing override keyword
            METHOD Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0505] Error: override violation
       ,-[ file:///test0.st:8:20 ]
       |
     3 |             METHOD Tick : INT END_METHOD
       |                    ^^|^
       |                      `--- base method 'Tick' is declared here
       |
     8 |             METHOD Tick : INT END_METHOD
       |                    ^^|^
       |                      `--- missing OVERRIDE keyword for method 'Tick'
       |
       | Note: OVERRIDE is required when redefining a method with the same signature from a base class or function block
    ---'
    ");
}

#[rstest]
fn override_final_method(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD FINAL Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // override of FINAL method
            METHOD OVERRIDE Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0504] Error: override violation
       ,-[ file:///test0.st:8:29 ]
       |
     3 |             METHOD FINAL Tick : INT END_METHOD
       |                          ^^|^
       |                            `--- FINAL method 'Tick' is declared here
       |
     8 |             METHOD OVERRIDE Tick : INT END_METHOD
       |                             ^^|^
       |                               `--- cannot override FINAL method 'Tick'
       |
       | Note: methods marked as FINAL cannot be overridden
    ---'
    ");
}

#[rstest]
fn missing_abstract_method(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD ABSTRACT Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // missing implementation of method
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: inheritance violation
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD ABSTRACT Tick : INT END_METHOD
       |                             ^^|^
       |                               `--- ABSTRACT method 'Tick' is declared here
       |
     6 |         CLASS Mid EXTENDS Base
       |               ^|^
       |                `--- missing implementation for ABSTRACT method 'Tick'
       |
       | Note: ABSTRACT methods must be implemented by derived POUs
    ---'
    ");
}

#[rstest]
fn empty_override(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD OVERRIDE Tick : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0507] Error: inheritance violation
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             METHOD OVERRIDE Tick : INT END_METHOD
       |                             ^^|^
       |                               `--- invalid usage of OVERRIDE for method 'Tick'
       |
       | Note: OVERRIDE is only valid when the method is inherited
    ---'
    ");
}

#[rstest]
fn abstract_class_has_no_abstract_methods(mut with_db: RootDatabase) {
    let source = r#"
        CLASS ABSTRACT Base
            METHOD Tick : INT END_METHOD
            METHOD Tick2 : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0508] Error: inheritance violation
       ,-[ file:///test0.st:2:24 ]
       |
     2 |         CLASS ABSTRACT Base
       |                        ^^|^
       |                          `--- ABSTRACT class 'Base' has no abstract methods
       |
       | Note: abstract classes must have at least one abstract method
    ---'
    ");
}

#[rstest]
fn interface_methods_not_implemented(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0509] Error: inheritance violation
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD DAYTIME END_METHOD
       |                    ^^^|^^^
       |                       `----- method 'DAYTIME' is declared by interface 'Mid' here
       |
     6 |         CLASS Mid IMPLEMENTS ROOM1
       |               ^|^
       |                `--- missing implementation for interface method 'DAYTIME'
    ---'
    ");
}

#[rstest]
fn method_signature_count_mismatch_in_implementer(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
                VAR_INPUT
                    value: INT
                END_VAR
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME

            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
        ,-[ file:///test0.st:11:29 ]
        |
      3 |             METHOD DAYTIME
        |                    ^^^|^^^
        |                       `----- base method 'DAYTIME' is declared here
        |
     11 |             METHOD OVERRIDE DAYTIME
        |                             ^^^|^^^
        |                                `----- invalid number of parameters for method 'DAYTIME': expected 1, got 0
    ----'
    ");
}

#[rstest]
fn method_signature_count_mismatch_in_base(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME
                VAR_INPUT
                    value: INT
                END_VAR
            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
       ,-[ file:///test0.st:8:29 ]
       |
     3 |             METHOD DAYTIME
       |                    ^^^|^^^
       |                       `----- base method 'DAYTIME' is declared here
       |
     8 |             METHOD OVERRIDE DAYTIME
       |                             ^^^|^^^
       |                                `----- invalid number of parameters for method 'DAYTIME': expected 0, got 1
    ---'
    ");
}

#[rstest]
fn method_signature_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE ROOM1
            METHOD DAYTIME
                VAR_INPUT
                    value: INT;
                    value2: INT;
                END_VAR
            END_METHOD
        END_INTERFACE

        CLASS Mid IMPLEMENTS ROOM1
            METHOD OVERRIDE DAYTIME
                VAR_INPUT
                    value: INT;
                    value2: REAL; // should be INT
                END_VAR
            END_METHOD
        END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
        ,-[ file:///test0.st:12:29 ]
        |
     12 |             METHOD OVERRIDE DAYTIME
        |                             ^^^|^^^
        |                                `----- method 'DAYTIME' has incompatible parameter types: expected 'INT', got 'REAL'
        |
        | Note: parameter types must match those of the base method
    ----'
    ");
}

#[rstest]
fn super_without_extends_clause_on_fb(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
            SUPER.something
        END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0513] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             SUPER.something
       |             ^^|^^
       |               `---- 'SUPER' used but no EXTENDS clause found on 'fb'
    ---'
    ");
}

#[rstest]
fn super_without_extends_clause_on_class(mut with_db: RootDatabase) {
    let source = r#"
        CLASS cls
           METHOD doSomething
                SUPER.something
           END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0513] Error: invalid use of SUPER or THIS
       ,-[ file:///test0.st:4:17 ]
       |
     4 |                 SUPER.something
       |                 ^^|^^
       |                   `---- 'SUPER' used but no EXTENDS clause found on 'cls'
    ---'
    ");
}

#[rstest]
fn interface_method_without_override_is_valid(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE IWorker
            METHOD DoWork : INT END_METHOD
        END_INTERFACE

        CLASS Worker IMPLEMENTS IWorker
            METHOD DoWork : INT END_METHOD
        END_CLASS"#;

    // Implementing an interface method does NOT require OVERRIDE
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn interface_method_with_override_is_also_valid(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE IWorker
            METHOD DoWork : INT END_METHOD
        END_INTERFACE

        CLASS Worker IMPLEMENTS IWorker
            METHOD OVERRIDE DoWork : INT END_METHOD
        END_CLASS"#;

    // Using OVERRIDE for an interface method is allowed but not required
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn abstract_method_without_override_is_valid(mut with_db: RootDatabase) {
    let source = r#"
        CLASS ABSTRACT Base
            METHOD ABSTRACT Tick : INT END_METHOD
        END_CLASS

        CLASS Derived EXTENDS Base
            METHOD Tick : INT END_METHOD
        END_CLASS"#;

    // Implementing an abstract method does NOT require OVERRIDE
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn abstract_method_with_override_is_also_valid(mut with_db: RootDatabase) {
    let source = r#"
        CLASS ABSTRACT Base
            METHOD ABSTRACT Tick : INT END_METHOD
        END_CLASS

        CLASS Derived EXTENDS Base
            METHOD OVERRIDE Tick : INT END_METHOD
        END_CLASS"#;

    // Using OVERRIDE for an abstract method is allowed but not required
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn concrete_method_still_requires_override(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD Tick : INT END_METHOD
        END_CLASS

        CLASS Derived EXTENDS Base
            METHOD Tick : INT END_METHOD
        END_CLASS"#;

    // Overriding a concrete method STILL requires OVERRIDE
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0505] Error: override violation
       ,-[ file:///test0.st:7:20 ]
       |
     3 |             METHOD Tick : INT END_METHOD
       |                    ^^|^
       |                      `--- base method 'Tick' is declared here
       |
     7 |             METHOD Tick : INT END_METHOD
       |                    ^^|^
       |                      `--- missing OVERRIDE keyword for method 'Tick'
       |
       | Note: OVERRIDE is required when redefining a method with the same signature from a base class or function block
    ---'
    ");
}

#[rstest]
fn interface_param_method_call_is_valid(mut with_db: RootDatabase) {
    // Design 1: an interface is allowed as a VAR_IN_OUT parameter, and calling a
    // method through it is a VALID, type-checked call — it monomorphizes to a
    // direct `FB#doThing` in Phase B. So it produces no diagnostics (the
    // codegen-not-yet-monomorphized state is a MIR concern, not a semantic one).
    let source = r#"
        INTERFACE IFoo
            METHOD doThing : INT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK FB IMPLEMENTS IFoo
            METHOD doThing : INT
                doThing := 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR_IN_OUT i : IFoo; END_VAR
            test := i.doThing();
        END_FUNCTION
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// The RETURN half of E0512: comparing only parameters let a prototype's
// return diverge from its implementation — invalid wasm for a lane change
// (INT vs REAL), silently wrong values for a same-lane one (INT vs DINT).

#[rstest]
fn interface_return_type_mismatch_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE Ifc
    METHOD M : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK fb IMPLEMENTS Ifc
    METHOD M : REAL
        M := 1.5;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
       ,-[ file:///test0.st:8:16 ]
       |
     3 |     METHOD M : INT
       |                ^|^
       |                 `--- base method 'M' declares its return type here
       |
     8 |     METHOD M : REAL
       |                ^^|^
       |                  `--- method 'M' has an incompatible return type: expected 'INT', got 'REAL'
       |
       | Note: the return type must match the base method's
    ---'
    ");
}

#[rstest]
fn interface_return_type_same_lane_divergence_is_refused(mut with_db: RootDatabase) {
    // INT and DINT share the i32 lane, so this one compiled to VALID wasm
    // and returned out-of-domain values instead of failing to load.
    let source = r#"
INTERFACE Ifc
    METHOD M : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK fb IMPLEMENTS Ifc
    METHOD M : DINT
        M := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
       ,-[ file:///test0.st:8:16 ]
       |
     3 |     METHOD M : INT
       |                ^|^
       |                 `--- base method 'M' declares its return type here
       |
     8 |     METHOD M : DINT
       |                ^^|^
       |                  `--- method 'M' has an incompatible return type: expected 'INT', got 'DINT'
       |
       | Note: the return type must match the base method's
    ---'
    ");
}

#[rstest]
fn override_return_type_mismatch_is_refused(mut with_db: RootDatabase) {
    // The same rule through EXTENDS: an OVERRIDE may not change the return.
    let source = r#"
FUNCTION_BLOCK base
    METHOD M : INT
        M := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
    METHOD OVERRIDE M : REAL
        M := 1.5;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0512] Error: method signature mismatch
       ,-[ file:///test0.st:9:25 ]
       |
     3 |     METHOD M : INT
       |                ^|^
       |                 `--- base method 'M' declares its return type here
       |
     9 |     METHOD OVERRIDE M : REAL
       |                         ^^|^
       |                           `--- method 'M' has an incompatible return type: expected 'INT', got 'REAL'
       |
       | Note: the return type must match the base method's
    ---'
    ");
}

#[rstest]
fn matching_return_types_stay_clean(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE Ifc
    METHOD M : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK fb IMPLEMENTS Ifc
    METHOD M : INT
        M := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
