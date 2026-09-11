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
    [E1112] Error: override violation
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
    [E1114] Error: override violation
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
        CLASS ABSTRACT Base
            METHOD ABSTRACT Tick : INT END_METHOD
        END_CLASS

        CLASS Mid EXTENDS Base
            // missing implementation of method
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1116] Error: inheritance violation
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
    [E1113] Error: inheritance violation
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
fn abstract_class_without_abstract_methods_is_valid(mut with_db: RootDatabase) {
    // Not an IEC rule: ABSTRACT only forbids instantiation, so a concrete-only
    // ABSTRACT class is the ordinary extend-only base type. A code refused it
    // and was retired.
    let source = r#"
        CLASS ABSTRACT Base
            METHOD Tick : INT END_METHOD
            METHOD Tick2 : INT END_METHOD
        END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
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
    [E1119] Error: inheritance violation
       ,-[ file:///test0.st:6:15 ]
       |
     3 |             METHOD DAYTIME END_METHOD
       |                    ^^^|^^^
       |                       `----- method 'DAYTIME' is declared by interface 'ROOM1' here
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
    [E1125] Error: method parameter count mismatch
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
    [E1125] Error: method parameter count mismatch
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
    [E1126] Error: method parameter type mismatch
        ,-[ file:///test0.st:15:29 ]
        |
      6 |                     value2: INT;
        |                             ^|^
        |                              `--- the base method declares 'value2' as 'INT' here
        |
     15 |                     value2: REAL; // should be INT
        |                             ^^|^
        |                               `--- parameter 'value2' of method 'DAYTIME' has an incompatible type: expected 'INT', got 'REAL'
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
    [E1107] Error: invalid use of SUPER or THIS
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
    [E1107] Error: invalid use of SUPER or THIS
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
    [E1112] Error: override violation
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

// The RETURN half of E1125: comparing only parameters let a prototype's
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
    [E1127] Error: method return type mismatch
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
    [E1127] Error: method return type mismatch
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
    [E1127] Error: method return type mismatch
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

#[rstest]
fn class_member_shadowing_is_refused(mut with_db: RootDatabase) {
    // The check was FB-gated at both ends: two CLASSes declaring `x` shared
    // one slot silently — SetB() changed what GetA() returned — and with
    // different types the module was invalid wasm at exit 0.
    let source = r#"
CLASS A
    VAR x : INT; END_VAR
END_CLASS

CLASS B EXTENDS A
    VAR x : REAL; END_VAR
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1115] Error: inheritance violation
       ,-[ file:///test0.st:7:9 ]
       |
     3 |     VAR x : INT; END_VAR
       |         |
       |         `-- inherited variable 'x' is declared here
       |
     7 |     VAR x : REAL; END_VAR
       |         |
       |         `-- variable 'x' is already declared in a base POU
       |
       | Note: variable names in a base and derived POU must be unique
    ---'
    ");
}

#[rstest]
fn inherited_parameters_bind_at_call_sites(mut with_db: RootDatabase) {
    // The call-site parameter list is the flattened EXTENDS view: naming an
    // inherited input or in_out is legal (this was E0801 + E0803).
    let source = r#"
FUNCTION_BLOCK BaseIO
    VAR_IN_OUT io : INT; END_VAR
    VAR_INPUT inp : INT; END_VAR
    io := io + inp;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivedIO EXTENDS BaseIO
    VAR_INPUT own : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
    VAR d : DerivedIO; x : INT; END_VAR
    d(io := x, inp := 1, own := 2);
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn omitted_inherited_var_in_out_is_reported(mut with_db: RootDatabase) {
    // An inherited VAR_IN_OUT is as required as an own one; omitting it used
    // to pass silently and the base body's writes vanished.
    let source = r#"
FUNCTION_BLOCK BaseIO
    VAR_IN_OUT io : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivedIO EXTENDS BaseIO
    VAR_INPUT own : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
    VAR d : DerivedIO; END_VAR
    d(own := 2);
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:12:5 ]
        |
      3 |     VAR_IN_OUT io : INT; END_VAR
        |                ^^^^|^^^
        |                    `----- parameter 'io' declared here
        |
     12 |     d(own := 2);
        |     |
        |     `-- call to 'DerivedIO' is missing 1 required parameter: 'io'
        |
        | Note: VAR_IN_OUT parameters bind to caller-side l-values and must always be supplied
    ----'
    ");
}

#[rstest]
fn redeclared_var_external_is_not_shadowing(mut with_db: RootDatabase) {
    // Two VAR_EXTERNALs name the same global; neither owns storage, and
    // redeclaring is the only way the derived body reaches the global.
    let source = r#"
CONFIGURATION Cfg
    VAR_GLOBAL g : INT; END_VAR
END_CONFIGURATION

FUNCTION_BLOCK BaseE
    VAR_EXTERNAL g : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerE EXTENDS BaseE
    VAR_EXTERNAL g : INT; END_VAR
    g := 1;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// ABSTRACT means "incomplete, extend me". Unenforced, none of it held: a
// concrete POU could declare a bodyless method, an ABSTRACT type could be
// instantiated, and calling the method returned 0 from a clean check.

#[rstest]
fn abstract_method_in_a_concrete_class_is_refused(mut with_db: RootDatabase) {
    let source = r#"
CLASS C
    METHOD ABSTRACT m : INT END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1117] Error: inheritance violation
       ,-[ file:///test0.st:2:7 ]
       |
     2 | CLASS C
       |       |
       |       `-- CLASS 'C' declares an ABSTRACT method, so it must be ABSTRACT itself
     3 |     METHOD ABSTRACT m : INT END_METHOD
       |                     |
       |                     `-- ABSTRACT method 'm' is declared here
       |
       | Note: an ABSTRACT method has no body, so every POU declaring one is incomplete
    ---'
    ");
}

#[rstest]
fn abstract_method_in_a_concrete_function_block_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK F
    METHOD ABSTRACT m : INT END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1117] Error: inheritance violation
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK F
       |                |
       |                `-- FUNCTION_BLOCK 'F' declares an ABSTRACT method, so it must be ABSTRACT itself
     3 |     METHOD ABSTRACT m : INT END_METHOD
       |                     |
       |                     `-- ABSTRACT method 'm' is declared here
       |
       | Note: an ABSTRACT method has no body, so every POU declaring one is incomplete
    ---'
    ");
}

#[rstest]
fn instantiating_an_abstract_class_is_refused(mut with_db: RootDatabase) {
    let source = r#"
CLASS ABSTRACT B
    METHOD ABSTRACT m : INT END_METHOD
END_CLASS

PROGRAM P
VAR b : B; END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1118] Error: inheritance violation
       ,-[ file:///test0.st:7:9 ]
       |
     2 | CLASS ABSTRACT B
       |                |
       |                `-- CLASS 'B' is declared ABSTRACT here
       |
     7 | VAR b : B; END_VAR
       |         |
       |         `-- cannot instantiate ABSTRACT CLASS 'B'
       |
       | Note: declare a variable of a derived type that implements it
    ---'
    ");
}

#[rstest]
fn an_abstract_derived_pou_may_leave_methods_unimplemented(mut with_db: RootDatabase) {
    // Passing the obligation down is what an abstract intermediate is FOR;
    // E1116 used to refuse it, which made abstract hierarchies unusable.
    let source = r#"
CLASS ABSTRACT B
    METHOD ABSTRACT m : INT END_METHOD
END_CLASS

CLASS ABSTRACT D EXTENDS B
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn a_reference_to_an_abstract_type_is_allowed(mut with_db: RootDatabase) {
    // A reference names some derived instance; it is not an instance.
    let source = r#"
CLASS ABSTRACT B
    METHOD ABSTRACT m : INT END_METHOD
END_CLASS

TYPE PB : REF_TO B; END_TYPE

PROGRAM P
VAR p : PB; END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn a_concrete_derived_class_is_instantiable(mut with_db: RootDatabase) {
    let source = r#"
CLASS ABSTRACT B
    METHOD ABSTRACT m : INT END_METHOD
END_CLASS

CLASS D EXTENDS B
    METHOD m : INT m := 1; END_METHOD
END_CLASS

PROGRAM P
VAR d : D; x : INT; END_VAR
    x := d.m();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn extending_a_final_class_is_refused(mut with_db: RootDatabase) {
    // The method-level FINAL rule was enforced (E1114) while the type-level
    // one was not, so FINAL on a CLASS header meant nothing.
    let source = r#"
CLASS FINAL B
    METHOD PUBLIC m : INT m := 1; END_METHOD
END_CLASS

CLASS D EXTENDS B
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1104] Error: inheritance violation
       ,-[ file:///test0.st:6:17 ]
       |
     2 | CLASS FINAL B
       |             |
       |             `-- CLASS 'B' is declared FINAL here
       |
     6 | CLASS D EXTENDS B
       |                 |
       |                 `-- 'D' cannot extend FINAL CLASS 'B'
       |
       | Note: FINAL declares a type complete: it may be used, but not extended
    ---'
    ");
}

#[rstest]
fn extending_a_final_function_block_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK FINAL B
END_FUNCTION_BLOCK

FUNCTION_BLOCK D EXTENDS B
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1104] Error: inheritance violation
       ,-[ file:///test0.st:5:26 ]
       |
     2 | FUNCTION_BLOCK FINAL B
       |                      |
       |                      `-- FUNCTION_BLOCK 'B' is declared FINAL here
       |
     5 | FUNCTION_BLOCK D EXTENDS B
       |                          |
       |                          `-- 'D' cannot extend FINAL FUNCTION_BLOCK 'B'
       |
       | Note: FINAL declares a type complete: it may be used, but not extended
    ---'
    ");
}

#[rstest]
fn a_final_class_is_still_usable(mut with_db: RootDatabase) {
    // FINAL closes the type to EXTENSION, not to use.
    let source = r#"
CLASS FINAL B
    METHOD PUBLIC m : INT m := 1; END_METHOD
END_CLASS

PROGRAM P
VAR b : B; x : INT; END_VAR
    x := b.m();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn a_final_derived_class_may_extend_an_open_base(mut with_db: RootDatabase) {
    let source = r#"
CLASS B
    METHOD PUBLIC m : INT m := 1; END_METHOD
END_CLASS

CLASS FINAL D EXTENDS B
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// A signature is matched by POSITION, and at each position the NAME and the
// SECTION are part of it, not only the type: an interface's `VAR_INPUT a`
// implemented as `VAR_IN_OUT a` was called by value through the interface and
// by address in the implementation — executed, the call returned a wrong
// value with no diagnostic. One complaint per position: a different name
// stops the comparison there, since the type would be checked against the
// wrong counterpart.

#[rstest]
fn interface_parameter_section_mismatch_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE I
    METHOD M : INT
        VAR_INPUT a : INT; END_VAR
    END_METHOD
END_INTERFACE

CLASS C IMPLEMENTS I
    METHOD M : INT
        VAR_IN_OUT a : INT; END_VAR
        M := a;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1129] Error: method parameter section mismatch
        ,-[ file:///test0.st:10:20 ]
        |
      4 |         VAR_INPUT a : INT; END_VAR
        |                   |
        |                   `-- the base method declares 'a' as VAR_INPUT here
        |
     10 |         VAR_IN_OUT a : INT; END_VAR
        |                    |
        |                    `-- parameter 'a' of method 'M' is VAR_IN_OUT here but VAR_INPUT in the base method
    ----'
    ");
}

#[rstest]
fn interface_parameter_name_mismatch_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE I
    METHOD M : INT
        VAR_INPUT a : INT; END_VAR
    END_METHOD
END_INTERFACE

CLASS C IMPLEMENTS I
    METHOD M : INT
        VAR_INPUT b : INT; END_VAR
        M := b;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1128] Error: method parameter name mismatch
        ,-[ file:///test0.st:10:19 ]
        |
      4 |         VAR_INPUT a : INT; END_VAR
        |                   |
        |                   `-- the base method declares 'a' at this position
        |
     10 |         VAR_INPUT b : INT; END_VAR
        |                   |
        |                   `-- parameter 'b' of method 'M' is named 'a' in the base method
    ----'
    ");
}

#[rstest]
fn override_parameter_section_mismatch_is_refused(mut with_db: RootDatabase) {
    let source = r#"
CLASS B
    METHOD M : INT
        VAR_INPUT a : INT; END_VAR
        M := a;
    END_METHOD
END_CLASS

CLASS D EXTENDS B
    METHOD OVERRIDE M : INT
        VAR_OUTPUT a : INT; END_VAR
        M := 0;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1129] Error: method parameter section mismatch
        ,-[ file:///test0.st:11:20 ]
        |
      4 |         VAR_INPUT a : INT; END_VAR
        |                   |
        |                   `-- the base method declares 'a' as VAR_INPUT here
        |
     11 |         VAR_OUTPUT a : INT; END_VAR
        |                    |
        |                    `-- parameter 'a' of method 'M' is VAR_OUTPUT here but VAR_INPUT in the base method
    ----'
    ");
}

#[rstest]
fn swapped_parameters_report_names_not_types(mut with_db: RootDatabase) {
    // `b : REAL, a : INT` against `a : INT, b : REAL`: two name mismatches,
    // and NOT two type mismatches on top of them.
    let source = r#"
INTERFACE I
    METHOD M : INT
        VAR_INPUT a : INT; b : REAL; END_VAR
    END_METHOD
END_INTERFACE

CLASS C IMPLEMENTS I
    METHOD M : INT
        VAR_INPUT b : REAL; a : INT; END_VAR
        M := a;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1128] Error: method parameter name mismatch
        ,-[ file:///test0.st:10:19 ]
        |
      4 |         VAR_INPUT a : INT; b : REAL; END_VAR
        |                   |
        |                   `-- the base method declares 'a' at this position
        |
     10 |         VAR_INPUT b : REAL; a : INT; END_VAR
        |                   |
        |                   `-- parameter 'b' of method 'M' is named 'a' in the base method
    ----'
    [E1128] Error: method parameter name mismatch
        ,-[ file:///test0.st:10:29 ]
        |
      4 |         VAR_INPUT a : INT; b : REAL; END_VAR
        |                            |
        |                            `-- the base method declares 'b' at this position
        |
     10 |         VAR_INPUT b : REAL; a : INT; END_VAR
        |                             |
        |                             `-- parameter 'a' of method 'M' is named 'b' in the base method
    ----'
    ");
}
