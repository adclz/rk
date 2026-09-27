use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// Design 1 (params-only): interface types are allowed ONLY as VAR_INPUT /
// VAR_IN_OUT parameters, where they are monomorphized to a concrete type.
// Everywhere else — stored VAR, FB members, VAR_OUTPUT, VAR_GLOBAL, VAR_TEMP —
// is rejected with E1121, so no interface value can outlive a call or be
// dispatched dynamically.
//
// The old suite tested interface *polymorphism* through STORED interface
// variables (assignment compatibility, method calls, interface-extends). Under
// Design 1 those are rejected; the working-dispatch and assignment-compat cases
// return in Phase B, rewritten around interface PARAMETERS once param calls are
// monomorphized. Interface RETURN types and nested `ARRAY OF ITF1` are not yet
// covered by E1121 (known follow-ups).

// --- E1121: interface types rejected outside VAR_INPUT / VAR_IN_OUT ---

#[rstest]
fn interface_stored_var_rejected(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        itf: ITF1;
    END_VAR
END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         itf: ITF1;
       |         ^|^
       |          `--- interface type 'ITF1' is not allowed in VAR
       |
       | Note: interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters
    ---'
    ");
}

#[rstest]
fn interface_fb_member_rejected(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Holder
    VAR
        dev: ITF1;
    END_VAR
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         dev: ITF1;
       |         ^|^
       |          `--- interface type 'ITF1' is not allowed in VAR
       |
       | Note: interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters
    ---'
    ");
}

#[rstest]
fn interface_output_rejected(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Producer
    VAR_OUTPUT
        out: ITF1;
    END_VAR
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         out: ITF1;
       |         ^|^
       |          `--- interface type 'ITF1' is not allowed in VAR_OUTPUT
       |
       | Note: interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters
    ---'
    ");
}

#[rstest]
fn interface_temp_rejected(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Use : INT
    VAR_TEMP
        t: ITF1;
    END_VAR
    Use := 0;
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         t: ITF1;
       |         |
       |         `-- interface type 'ITF1' is not allowed in VAR_TEMP
       |
       | Note: interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters
    ---'
    ");
}

#[rstest]
fn interface_array_rejected(mut with_db: RootDatabase) {
    // Nested interface: `ARRAY OF ITF1` is a stored, heterogeneous collection —
    // rejected, since a per-element concrete type can't be monomorphized.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        arr: ARRAY[0..2] OF ITF1;
    END_VAR
END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1123] Error: interface type not allowed here
       ,-[ file:///test0.st:8:14 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         arr: ARRAY[0..2] OF ITF1;
       |              ^^^^^^^^^|^^^^^^^^^
       |                       `----------- interface type 'ITF1' cannot be nested inside another type (array, reference, or struct)
       |
       | Note: an interface may only appear directly as a VAR_INPUT or VAR_IN_OUT parameter
    ---'
    ");
}

#[rstest]
fn interface_return_type_rejected(mut with_db: RootDatabase) {
    // An interface return type flows the concrete type callee→caller and can't
    // be monomorphized.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Make : ITF1
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1122] Error: interface type not allowed here
       ,-[ file:///test0.st:6:17 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     6 | FUNCTION Make : ITF1
       |                 ^^|^
       |                   `--- interface type 'ITF1' is not allowed as a return type
       |
       | Note: interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters
    ---'
    ");
}

// --- allowed: interface as VAR_INPUT / VAR_IN_OUT parameter (no E1121) ---

#[rstest]
fn interface_named_struct_field_rejected(mut with_db: RootDatabase) {
    // An interface as a field of a named STRUCT type is stored state → rejected
    // at the struct's definition.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

TYPE Holder :
    STRUCT
        dev: ITF1;
    END_STRUCT
END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1123] Error: interface type not allowed here
       ,-[ file:///test0.st:8:14 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         dev: ITF1;
       |              ^^|^
       |                `--- interface type 'ITF1' cannot be nested inside another type (array, reference, or struct)
       |
       | Note: an interface may only appear directly as a VAR_INPUT or VAR_IN_OUT parameter
    ---'
    ");
}

#[rstest]
fn interface_inline_struct_field_rejected(mut with_db: RootDatabase) {
    // Same for an inline struct in a variable declaration.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM A
    VAR
        s: STRUCT dev: ITF1; END_STRUCT;
    END_VAR
END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1123] Error: interface type not allowed here
       ,-[ file:///test0.st:8:24 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         s: STRUCT dev: ITF1; END_STRUCT;
       |                        ^^|^
       |                          `--- interface type 'ITF1' cannot be nested inside another type (array, reference, or struct)
       |
       | Note: an interface may only appear directly as a VAR_INPUT or VAR_IN_OUT parameter
    ---'
    ");
}

#[rstest]
fn interface_array_param_still_rejected(mut with_db: RootDatabase) {
    // A NESTED interface is rejected even as a parameter: `ARRAY OF ITF1` is
    // heterogeneous and can't be monomorphized, unlike a direct interface param.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Use : INT
    VAR_INPUT
        arr: ARRAY[0..2] OF ITF1;
    END_VAR
    Use := 0;
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1123] Error: interface type not allowed here
       ,-[ file:///test0.st:8:14 ]
       |
     2 | INTERFACE ITF1
       |           ^^|^
       |             `--- interface 'ITF1' is defined here
       |
     8 |         arr: ARRAY[0..2] OF ITF1;
       |              ^^^^^^^^^|^^^^^^^^^
       |                       `----------- interface type 'ITF1' cannot be nested inside another type (array, reference, or struct)
       |
       | Note: an interface may only appear directly as a VAR_INPUT or VAR_IN_OUT parameter
    ---'
    ");
}

#[rstest]
fn interface_input_param_allowed(mut with_db: RootDatabase) {
    // Declaring an interface INPUT parameter is allowed. (Calling a method on it
    // is monomorphized in Phase B; here we only exercise the declaration.)
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Use : INT
    VAR_INPUT
        dev: ITF1;
    END_VAR
    Use := 0;
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An FB or PROGRAM input lives in the instance across calls, which makes an
// interface there STORED state; parameters specialize per call, so they
// exist on FUNCTION and METHOD only. These shapes used to pass `rk check`
// and ICE in `rk compile` with "Unsupported type: Interface".
#[rstest]
fn interface_inout_on_fb_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Runner
    VAR_IN_OUT
        dev: ITF1;
    END_VAR
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         dev: ITF1;
       |         ^^^^|^^^^
       |             `------ interface 'ITF1' cannot be a FUNCTION_BLOCK VAR_IN_OUT: the instance would store it across calls; interface parameters exist on FUNCTION and METHOD only
    ---'
    ");
}

#[rstest]
fn interface_input_on_fb_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Runner
    VAR_INPUT
        dev: ITF1;
    END_VAR
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         dev: ITF1;
       |         ^^^^|^^^^
       |             `------ interface 'ITF1' cannot be a FUNCTION_BLOCK VAR_INPUT: the instance would store it across calls; interface parameters exist on FUNCTION and METHOD only
    ---'
    ");
}

#[rstest]
fn interface_input_on_program_is_refused(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM P
    VAR_INPUT
        dev: ITF1;
    END_VAR
END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1121] Error: interface type not allowed here
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         dev: ITF1;
       |         ^^^^|^^^^
       |             `------ interface 'ITF1' cannot be a PROGRAM VAR_INPUT: the instance would store it across calls; interface parameters exist on FUNCTION and METHOD only
    ---'
    ");
}

// The transient forms stay legal: a METHOD's interface params specialize per
// call even though the method lives on an FB.
#[rstest]
fn interface_param_on_fb_method_stays_allowed(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Runner
    METHOD PUBLIC Use : INT
        VAR_INPUT dev: ITF1; END_VAR
        VAR_IN_OUT alt: ITF1; END_VAR
        Use := 0;
    END_METHOD
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// --- unrelated: using an interface as a value directly is still a type error ---

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
    [E0317] Error: semantic violation
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
fn interface_param_reassignment_rejected(mut with_db: RootDatabase) {
    // An interface VAR_IN_OUT param is a fixed binding to the caller's concrete
    // type; reassigning it would break monomorphization (the body is specialized
    // to one concrete type) → E1124.
    let source = r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Use : INT
    VAR_IN_OUT a : ITF1; b : ITF1; END_VAR
    a := b;
    Use := 0;
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1124] Error: interface parameter is not assignable
       ,-[ file:///test0.st:8:5 ]
       |
     7 |     VAR_IN_OUT a : ITF1; b : ITF1; END_VAR
       |                |
       |                `-- interface parameter 'a' is declared here
     8 |     a := b;
       |     |
       |     `-- cannot assign to interface parameter 'a'
       |
       | Note: an interface parameter is a fixed binding to the concrete type passed by the caller; it can be used (methods called, passed on) but not reassigned
    ---'
    ");
}

// `THIS` is a VALUE — the instance a body runs on — so it may be passed where
// an interface parameter expects an implementer. It types as the FB itself,
// the same type a bare NAME types as, so it reaches the ordinary coercion and
// is accepted or refused there rather than by the type-used-as-a-value check.
#[rstest]
fn valid_this_as_interface_argument(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE IWork
            METHOD Run : INT END_METHOD
        END_INTERFACE
        FUNCTION drive : INT
            VAR_IN_OUT dev : IWork; END_VAR
            drive := dev.Run();
        END_FUNCTION
        FUNCTION_BLOCK Worker IMPLEMENTS IWork
            METHOD Run : INT  Run := 1; END_METHOD
            METHOD Go : INT   Go := drive(dev := THIS); END_METHOD
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An FB that does NOT implement the interface is still refused — the coercion
// decides, so the diagnostic is a mismatch rather than a misuse of a type.
#[rstest]
fn invalid_this_as_interface_argument_not_implemented(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE IWork
            METHOD Run : INT END_METHOD
        END_INTERFACE
        FUNCTION drive : INT
            VAR_IN_OUT dev : IWork; END_VAR
            drive := dev.Run();
        END_FUNCTION
        FUNCTION_BLOCK Idle
            METHOD Go : INT  Go := drive(dev := THIS); END_METHOD
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:49 ]
        |
      2 |         INTERFACE IWork
        |                   ^^|^^
        |                     `---- INTERFACE 'IWork' is defined here
        |
     10 |             METHOD Go : INT  Go := drive(dev := THIS); END_METHOD
        |                                                 ^^|^
        |                                                   `--- expected 'IWork', got 'Idle'
    ----'
    ");
}

// `THIS` is not assignable: an instance cannot be rebound to another, which
// would copy one instance's state over another's. The check on the assignment
// TARGET says so, and now says it alone — it used to be followed by a type
// mismatch reading "expected 'Worker', got 'Worker'".
#[rstest]
fn invalid_this_assigned_to_variable(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Worker
        VAR other : Worker; END_VAR
            METHOD Go
                other := THIS;
            END_METHOD
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:24 ]
       |
     2 |         FUNCTION_BLOCK Worker
       |                        ^^^|^^
       |                           `---- type 'Worker' is recursive (contains itself)
     3 |         VAR other : Worker; END_VAR
       |                     ^^^|^^
       |                        `---- 'Worker' references itself here
    ---'
    [E0318] Error: semantic violation
       ,-[ file:///test0.st:5:17 ]
       |
     5 |                 other := THIS;
       |                 ^^|^^
       |                   `---- 'Worker' is a callable type and can not be assigned
    ---'
    ");
}
