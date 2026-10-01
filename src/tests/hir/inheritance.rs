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
        |                       `----- interface method 'DAYTIME' is declared here
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
       |                       `----- interface method 'DAYTIME' is declared here
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
        |                              `--- the interface method declares 'value2' as 'INT' here
        |
     15 |                     value2: REAL; // should be INT
        |                             ^^|^
        |                               `--- parameter 'value2' of method 'DAYTIME' has an incompatible type: expected 'INT', got 'REAL'
        |
        | Note: parameter types must match those of the interface method
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
       |                 `--- interface method 'M' declares its return type here
       |
     8 |     METHOD M : REAL
       |                ^^|^
       |                  `--- method 'M' has an incompatible return type: expected 'INT', got 'REAL'
       |
       | Note: the return type must match the interface method's
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
       |                 `--- interface method 'M' declares its return type here
       |
     8 |     METHOD M : DINT
       |                ^^|^
       |                  `--- method 'M' has an incompatible return type: expected 'INT', got 'DINT'
       |
       | Note: the return type must match the interface method's
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
       | Note: use a derived type that implements it
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
    // A reference is not an instance, so declaring one is not refused. It can
    // hold only NULL: no instance of an ABSTRACT type exists, and a derived
    // one does not bind a base reference (E0301,
    // `a_derived_instance_binds_no_base_reference_or_parameter`).
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

/// A derived instance binds no base-typed reference or parameter, so a
/// method called through one always runs on a base instance: the base's copy
/// of the method, where THIS is the base, is the right one. Nothing is looked
/// up at run time, and nothing has to be.
#[rstest]
fn a_derived_instance_binds_no_base_reference_or_parameter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK ABSTRACT Shape
    METHOD PUBLIC ABSTRACT Area : INT END_METHOD
    METHOD PUBLIC Describe : INT Describe := 1000 + THIS.Area(); END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Square EXTENDS Shape
    METHOD PUBLIC OVERRIDE Area : INT Area := 16; END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Base
    METHOD PUBLIC Hook : INT Hook := 1; END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Hook : INT Hook := 2; END_METHOD
END_FUNCTION_BLOCK
FUNCTION TakeBase : INT VAR_IN_OUT b : Base; END_VAR TakeBase := b.Hook(); END_FUNCTION

FUNCTION_BLOCK User
VAR q : Square; rs : REF_TO Shape; d : Derived; rb : REF_TO Base; x : INT; END_VAR
    rs := REF(q);
    rb := REF(d);
    x := TakeBase(b := d);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:19:11 ]
        |
     18 | VAR q : Square; rs : REF_TO Shape; d : Derived; rb : REF_TO Base; x : INT; END_VAR
        |                 ^|
        |                  `-- type is declared by variable 'rs' here
     19 |     rs := REF(q);
        |           ^^^|^^
        |              `---- expected 'REF_TO Shape', got 'REF_TO Square'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:11 ]
        |
     18 | VAR q : Square; rs : REF_TO Shape; d : Derived; rb : REF_TO Base; x : INT; END_VAR
        |                                                 ^|
        |                                                  `-- type is declared by variable 'rb' here
        |
     20 |     rb := REF(d);
        |           ^^^|^^
        |              `---- expected 'REF_TO Base', got 'REF_TO Derived'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:21:24 ]
        |
      9 | FUNCTION_BLOCK Base
        |                ^^|^
        |                  `--- FUNCTION_BLOCK 'Base' is defined here
        |
     21 |     x := TakeBase(b := d);
        |                        |
        |                        `-- expected 'Base', got 'Derived'
    ----'
    ");
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
        |                   `-- the interface method declares 'a' as VAR_INPUT here
        |
     10 |         VAR_IN_OUT a : INT; END_VAR
        |                    |
        |                    `-- parameter 'a' of method 'M' is VAR_IN_OUT here but VAR_INPUT in the interface method
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
        |                   `-- the interface method declares 'a' at this position
        |
     10 |         VAR_INPUT b : INT; END_VAR
        |                   |
        |                   `-- parameter 'b' of method 'M' is named 'a' in the interface method
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
        |                   `-- the interface method declares 'a' at this position
        |
     10 |         VAR_INPUT b : REAL; a : INT; END_VAR
        |                   |
        |                   `-- parameter 'b' of method 'M' is named 'a' in the interface method
    ----'
    [E1128] Error: method parameter name mismatch
        ,-[ file:///test0.st:10:29 ]
        |
      4 |         VAR_INPUT a : INT; b : REAL; END_VAR
        |                            |
        |                            `-- the interface method declares 'b' at this position
        |
     10 |         VAR_INPUT b : REAL; a : INT; END_VAR
        |                             |
        |                             `-- parameter 'a' of method 'M' is named 'b' in the interface method
    ----'
    ");
}

/// A base of the wrong kind is refused where it is written, and is no base:
/// an FB extending an INTERFACE no longer owes its methods (the E1119 it
/// used to get). An FB may extend a CLASS; a CLASS may not extend an FB.
#[rstest]
fn a_base_of_the_wrong_kind_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE I
    METHOD m : INT END_METHOD
END_INTERFACE

CLASS C
END_CLASS

FUNCTION_BLOCK F
END_FUNCTION_BLOCK

FUNCTION_BLOCK ExtendsInterface EXTENDS I
END_FUNCTION_BLOCK

CLASS ImplementsClass IMPLEMENTS C
END_CLASS

INTERFACE ExtendsBlock EXTENDS F
END_INTERFACE

CLASS ClassExtendsBlock EXTENDS F
END_CLASS

CLASS ClassImplementsBlock IMPLEMENTS F
END_CLASS

FUNCTION_BLOCK BlockExtendsClass EXTENDS C
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1130] Error: base of the wrong kind
        ,-[ file:///test0.st:12:41 ]
        |
      2 | INTERFACE I
        |           |
        |           `-- INTERFACE 'I' is declared here
        |
     12 | FUNCTION_BLOCK ExtendsInterface EXTENDS I
        |                                         |
        |                                         `-- FUNCTION_BLOCK 'ExtendsInterface' cannot extend INTERFACE 'I': a FUNCTION_BLOCK extends FUNCTION_BLOCKs and CLASSes
        |
        | Note: an INTERFACE is implemented, with IMPLEMENTS
    ----'
    [E1130] Error: base of the wrong kind
        ,-[ file:///test0.st:15:34 ]
        |
      6 | CLASS C
        |       |
        |       `-- CLASS 'C' is declared here
        |
     15 | CLASS ImplementsClass IMPLEMENTS C
        |                                  |
        |                                  `-- CLASS 'ImplementsClass' cannot implement CLASS 'C': IMPLEMENTS names an INTERFACE
        |
        | Note: CLASS 'C' is extended, with EXTENDS
    ----'
    [E1130] Error: base of the wrong kind
        ,-[ file:///test0.st:18:32 ]
        |
      9 | FUNCTION_BLOCK F
        |                |
        |                `-- FUNCTION_BLOCK 'F' is declared here
        |
     18 | INTERFACE ExtendsBlock EXTENDS F
        |                                |
        |                                `-- INTERFACE 'ExtendsBlock' cannot extend FUNCTION_BLOCK 'F': an INTERFACE extends INTERFACEs
    ----'
    [E1130] Error: base of the wrong kind
        ,-[ file:///test0.st:21:33 ]
        |
      9 | FUNCTION_BLOCK F
        |                |
        |                `-- FUNCTION_BLOCK 'F' is declared here
        |
     21 | CLASS ClassExtendsBlock EXTENDS F
        |                                 |
        |                                 `-- CLASS 'ClassExtendsBlock' cannot extend FUNCTION_BLOCK 'F': a CLASS extends CLASSes
    ----'
    [E1130] Error: base of the wrong kind
        ,-[ file:///test0.st:24:39 ]
        |
      9 | FUNCTION_BLOCK F
        |                |
        |                `-- FUNCTION_BLOCK 'F' is declared here
        |
     24 | CLASS ClassImplementsBlock IMPLEMENTS F
        |                                       |
        |                                       `-- CLASS 'ClassImplementsBlock' cannot implement FUNCTION_BLOCK 'F': IMPLEMENTS names an INTERFACE
    ----'
    ");
}

/// An FB implements what its bases implement, and PROTECTED reaches a base
/// at any depth: both only looked at the POU's own header.
#[rstest]
fn inherited_interfaces_and_protected_reach_every_derived_pou(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IShow
    METHOD show : INT END_METHOD
END_INTERFACE

FUNCTION_BLOCK Base IMPLEMENTS IShow
    METHOD PUBLIC show : INT
        show := 1;
    END_METHOD
    METHOD PROTECTED hidden : INT
        hidden := 2;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Middle EXTENDS Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK Leaf EXTENDS Middle
    METHOD PUBLIC reach : INT
        reach := THIS.hidden();
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION ask : INT
VAR_INPUT dev : IShow; END_VAR
    ask := dev.show();
END_FUNCTION

FUNCTION run : INT
VAR l : Leaf; END_VAR
    run := ask(dev := l);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A cycle of bases beside a syntax error: salsa refused the cycle of
/// `ancestry` once the file's parse errors were accumulated, and panicked.
#[rstest]
fn a_cycle_of_bases_beside_a_syntax_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK A EXTENDS B
END_FUNCTION_BLOCK

FUNCTION_BLOCK B EXTENDS A
END_FUNCTION_BLOCK

@@@ garbage ###
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK A EXTENDS B
       |                |
       |                `-- type 'A' is recursive
       |
     5 | FUNCTION_BLOCK B EXTENDS A
       |                          |
       |                          `-- recurses at this location
       |
       | Note: cycle goes
       |       -> A
       |       -> B
       |       ... and back to A
    ---'
    [E0001] Error: syntax
       ,-[ file:///test0.st:8:1 ]
       |
     8 | @@@ garbage ###
       | ^^^^^^^|^^^^^^^
       |        `--------- Unexpected token(s): '@@@ garbage # # #'
    ---'
    ");
}

/// A cycle of EXTENDS is reported once and nothing follows from it: the
/// members and methods of every POU in it still resolve. Cyclic interfaces
/// overflowed the compiler's stack when one was converted to another.
#[rstest]
fn a_cycle_of_bases_is_reported_once(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK A EXTENDS B
VAR x : INT; END_VAR
    METHOD PUBLIC ma : INT
        ma := x + y;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK B EXTENDS A
VAR y : INT; END_VAR
    METHOD PUBLIC mb : INT
        mb := THIS.ma();
    END_METHOD
END_FUNCTION_BLOCK

INTERFACE IA EXTENDS IB
    METHOD m : INT END_METHOD
END_INTERFACE

INTERFACE IB EXTENDS IA
    METHOD n : INT END_METHOD
END_INTERFACE

FUNCTION take_b : INT
VAR_INPUT i : IB; END_VAR
    take_b := i.m() + i.n();
END_FUNCTION

FUNCTION pass_a : INT
VAR_INPUT i : IA; END_VAR
    pass_a := take_b(i := i);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK A EXTENDS B
       |                |
       |                `-- type 'A' is recursive
       |
     9 | FUNCTION_BLOCK B EXTENDS A
       |                          |
       |                          `-- recurses at this location
       |
       | Note: cycle goes
       |       -> A
       |       -> B
       |       ... and back to A
    ---'
    [E1302] Error: recursion detected
        ,-[ file:///test0.st:16:11 ]
        |
     16 | INTERFACE IA EXTENDS IB
        |           ^|
        |            `-- type 'IA' is recursive
        |
     20 | INTERFACE IB EXTENDS IA
        |                      ^|
        |                       `-- recurses at this location
        |
        | Note: cycle goes
        |       -> IA
        |       -> IB
        |       ... and back to IA
    ----'
    ");
}

/// An INTERFACE met by an inherited method is checked against it too, at
/// the IMPLEMENTS that asks for it. It used to pass, and the call through
/// the interface read an INT where it promised an LREAL: an invalid module.
#[rstest]
fn an_inherited_implementation_is_checked_against_the_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IMeasure
    METHOD Value : LREAL END_METHOD
END_INTERFACE

FUNCTION_BLOCK Base
    METHOD PUBLIC Value : INT
        Value := 3;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Sensor EXTENDS Base IMPLEMENTS IMeasure
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1127] Error: method return type mismatch
        ,-[ file:///test0.st:12:47 ]
        |
      3 |     METHOD Value : LREAL END_METHOD
        |                    ^^|^^
        |                      `---- interface method 'Value' declares its return type here
        |
      7 |     METHOD PUBLIC Value : INT
        |                   ^^|^^
        |                     `---- 'Value' is inherited from 'Base', declared here
        |
     12 | FUNCTION_BLOCK Sensor EXTENDS Base IMPLEMENTS IMeasure
        |                                               ^^^^|^^^
        |                                                   `----- method 'Value' has an incompatible return type: expected 'LREAL', got 'INT'
        |
        | Note: the return type must match the interface method's
    ----'
    ");
}

/// The signature is the parameters: an override's or an implementation's
/// own VAR and VAR_TEMP are not part of it.
#[rstest]
fn locals_are_no_part_of_a_signature(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
    METHOD PUBLIC Calc : INT
    VAR_INPUT x : INT; END_VAR
        Calc := x;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Calc : INT
    VAR_INPUT x : INT; END_VAR
    VAR tmp : INT; END_VAR
        tmp := x * 2;
        Calc := tmp;
    END_METHOD
END_FUNCTION_BLOCK

INTERFACE ICalc
    METHOD Calc : INT
    VAR_INPUT x : INT; END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Impl IMPLEMENTS ICalc
    METHOD PUBLIC Calc : INT
    VAR_INPUT x : INT; END_VAR
    VAR_TEMP t : INT; END_VAR
        t := x + 1;
        Calc := t;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// One prototype reached by two paths is one method: through two
/// interfaces extending the same one, or named directly and again through
/// an interface that extends it.
#[rstest]
fn an_interface_reached_twice_is_one_method(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IBase
    METHOD Id : INT END_METHOD
END_INTERFACE

INTERFACE IRead EXTENDS IBase
    METHOD Read : INT END_METHOD
END_INTERFACE

INTERFACE IWrite EXTENDS IBase
    METHOD Write : INT END_METHOD
END_INTERFACE

FUNCTION_BLOCK Diamond IMPLEMENTS IRead, IWrite
    METHOD PUBLIC Id : INT Id := 5; END_METHOD
    METHOD PUBLIC Read : INT Read := 9; END_METHOD
    METHOD PUBLIC Write : INT Write := 8; END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Twice IMPLEMENTS IRead, IBase
    METHOD PUBLIC Id : INT Id := 5; END_METHOD
    METHOD PUBLIC Read : INT Read := 9; END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// Every instance of an ABSTRACT type is refused: an array's elements at
/// any depth, a FUNCTION's or a METHOD's result, and a named type or a
/// STRUCT field that holds one, at its declaration, where the fix goes.
/// A variable of such a type is not refused again.
#[rstest]
fn every_instance_of_an_abstract_type_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK ABSTRACT Shape
    METHOD PUBLIC ABSTRACT Area : INT
    END_METHOD
END_FUNCTION_BLOCK

TYPE Shapes : ARRAY[0..1] OF Shape; END_TYPE
TYPE Holder : STRUCT s : Shape; END_STRUCT END_TYPE

FUNCTION MakeShape : Shape
END_FUNCTION

FUNCTION_BLOCK Factory
    METHOD PUBLIC Make : Shape
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM P
VAR
    grid : ARRAY[0..1, 0..1] OF Shape;
    named : Shapes;
END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1118] Error: inheritance violation
       ,-[ file:///test0.st:7:15 ]
       |
     2 | FUNCTION_BLOCK ABSTRACT Shape
       |                         ^^|^^
       |                           `---- FUNCTION_BLOCK 'Shape' is declared ABSTRACT here
       |
     7 | TYPE Shapes : ARRAY[0..1] OF Shape; END_TYPE
       |               ^^^^^^^^^^|^^^^^^^^^
       |                         `----------- cannot instantiate ABSTRACT FUNCTION_BLOCK 'Shape'
       |
       | Note: use a derived type that implements it
    ---'
    [E1118] Error: inheritance violation
       ,-[ file:///test0.st:8:26 ]
       |
     2 | FUNCTION_BLOCK ABSTRACT Shape
       |                         ^^|^^
       |                           `---- FUNCTION_BLOCK 'Shape' is declared ABSTRACT here
       |
     8 | TYPE Holder : STRUCT s : Shape; END_STRUCT END_TYPE
       |                          ^^|^^
       |                            `---- cannot instantiate ABSTRACT FUNCTION_BLOCK 'Shape'
       |
       | Note: use a derived type that implements it
    ---'
    [E1118] Error: inheritance violation
        ,-[ file:///test0.st:10:22 ]
        |
      2 | FUNCTION_BLOCK ABSTRACT Shape
        |                         ^^|^^
        |                           `---- FUNCTION_BLOCK 'Shape' is declared ABSTRACT here
        |
     10 | FUNCTION MakeShape : Shape
        |                      ^^|^^
        |                        `---- cannot instantiate ABSTRACT FUNCTION_BLOCK 'Shape'
        |
        | Note: use a derived type that implements it
    ----'
    [E1118] Error: inheritance violation
        ,-[ file:///test0.st:14:26 ]
        |
      2 | FUNCTION_BLOCK ABSTRACT Shape
        |                         ^^|^^
        |                           `---- FUNCTION_BLOCK 'Shape' is declared ABSTRACT here
        |
     14 |     METHOD PUBLIC Make : Shape
        |                          ^^|^^
        |                            `---- cannot instantiate ABSTRACT FUNCTION_BLOCK 'Shape'
        |
        | Note: use a derived type that implements it
    ----'
    [E1118] Error: inheritance violation
        ,-[ file:///test0.st:20:12 ]
        |
      2 | FUNCTION_BLOCK ABSTRACT Shape
        |                         ^^|^^
        |                           `---- FUNCTION_BLOCK 'Shape' is declared ABSTRACT here
        |
     20 |     grid : ARRAY[0..1, 0..1] OF Shape;
        |            ^^^^^^^^^^^^^|^^^^^^^^^^^^
        |                         `-------------- cannot instantiate ABSTRACT FUNCTION_BLOCK 'Shape'
        |
        | Note: use a derived type that implements it
    ----'
    ");
}

/// `SUPER.m()` on an ABSTRACT method has no body to run: it returned 0. An
/// ABSTRACT method with statements is refused too, since none would run.
#[rstest]
fn an_abstract_method_has_no_body_to_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK ABSTRACT Shape
    METHOD PUBLIC ABSTRACT Area : INT
    END_METHOD
    METHOD PUBLIC ABSTRACT Perimeter : INT
        Perimeter := 5;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Square EXTENDS Shape
    METHOD PUBLIC OVERRIDE Area : INT
        Area := SUPER.Area() + 4;
    END_METHOD
    METHOD PUBLIC OVERRIDE Perimeter : INT
        Perimeter := 16;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1134] Error: inheritance violation
       ,-[ file:///test0.st:5:28 ]
       |
     5 |     METHOD PUBLIC ABSTRACT Perimeter : INT
       |                            ^^^^|^^^^
       |                                `------ ABSTRACT method 'Perimeter' cannot have a body
       |
       | Note: a derived block implements it; without ABSTRACT, this body is the method's
    ---'
    [E1133] Error: invalid use of SUPER or THIS
        ,-[ file:///test0.st:12:23 ]
        |
      3 |     METHOD PUBLIC ABSTRACT Area : INT
        |                            ^^|^
        |                              `--- 'Area' is declared here
        |
     12 |         Area := SUPER.Area() + 4;
        |                       ^^|^
        |                         `--- SUPER.Area() calls the base's 'Area', which is ABSTRACT: it has no body to run
    ----'
    ");
}
