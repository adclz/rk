use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// Design 1 (params-only): interface types are allowed ONLY as VAR_INPUT /
// VAR_IN_OUT parameters, where they are monomorphized to a concrete type.
// Everywhere else — stored VAR, FB members, VAR_OUTPUT, VAR_GLOBAL, VAR_TEMP —
// is rejected with E0514, so no interface value can outlive a call or be
// dispatched dynamically.
//
// The old suite tested interface *polymorphism* through STORED interface
// variables (assignment compatibility, method calls, interface-extends). Under
// Design 1 those are rejected; the working-dispatch and assignment-compat cases
// return in Phase B, rewritten around interface PARAMETERS once param calls are
// monomorphized. Interface RETURN types and nested `ARRAY OF ITF1` are not yet
// covered by E0514 (known follow-ups).

// --- E0514: interface types rejected outside VAR_INPUT / VAR_IN_OUT ---

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
    [E0514] Error: interface type not allowed here
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
    [E0514] Error: interface type not allowed here
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
    [E0514] Error: interface type not allowed here
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
    [E0514] Error: interface type not allowed here
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
    [E0516] Error: interface type not allowed here
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
    [E0515] Error: interface type not allowed here
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

// --- allowed: interface as VAR_INPUT / VAR_IN_OUT parameter (no E0514) ---

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
    [E0516] Error: interface type not allowed here
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
    [E0516] Error: interface type not allowed here
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
    [E0516] Error: interface type not allowed here
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

#[rstest]
fn interface_inout_param_allowed(mut with_db: RootDatabase) {
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
