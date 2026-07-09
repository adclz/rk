// MIR identifier qualification by namespace path.
//
// Two POUs with the same bare name in different namespaces must not
// collide at the MIR level. Every MIR identifier (function name,
// instance type name, body/method name, FB struct mangling, ANY_*
// monomorphized suffixes, etc.) is qualified with the enclosing
// namespace path so each POU gets a unique slot in `function_indices`.
//
// This module exercises the qualification across every kind of
// exportable POU: FUNCTION, FUNCTION_BLOCK (and its methods), CLASS
// (and its methods), and PROGRAM. It also covers the interaction with
// ANY_* monomorphization mangling (`<Ns>.<Fb>$<T>`).

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

// -- FUNCTION -----------------------------------------------------------

#[rstest]
fn function_same_name_in_different_namespaces(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE NsA
    FUNCTION foo : INT
    VAR_INPUT x : INT; END_VAR
        foo := x;
    END_FUNCTION
END_NAMESPACE

NAMESPACE NsB
    FUNCTION foo : DINT
    VAR_INPUT x : DINT; END_VAR
        foo := x;
    END_FUNCTION
END_NAMESPACE

FUNCTION test
VAR i : INT; d : DINT; END_VAR
    i := NsA.foo(x := 10);
    d := NsB.foo(x := DINT#100);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export NsA.foo(Int) -> Int
    export NsB.foo(DInt) -> DInt
    export test()
    ");
}

// -- FUNCTION_BLOCK ----------------------------------------------------

#[rstest]
fn fb_same_name_in_different_namespaces(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE NsA
    FUNCTION_BLOCK Counter
    VAR_INPUT  PV : INT; END_VAR
    VAR_OUTPUT CV : INT; END_VAR
        CV := PV;
    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE NsB
    FUNCTION_BLOCK Counter
    VAR_INPUT  PV : DINT; END_VAR
    VAR_OUTPUT CV : DINT; END_VAR
        CV := PV;
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION test
VAR
    c_a : NsA.Counter;
    c_b : NsB.Counter;
END_VAR
    c_a(PV := 10);
    c_b(PV := DINT#100);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export NsA.Counter$__body__(*struct(NsA.Counter))
    export NsB.Counter$__body__(*struct(NsB.Counter))
    export test()
    ");
}

#[rstest]
fn fb_with_method_same_name_in_different_namespaces(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE NsA
    FUNCTION_BLOCK Counter
    VAR_INPUT  PV : INT; END_VAR
    VAR_OUTPUT CV : INT; END_VAR
        METHOD reset
            CV := 0;
        END_METHOD
    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE NsB
    FUNCTION_BLOCK Counter
    VAR_INPUT  PV : DINT; END_VAR
    VAR_OUTPUT CV : DINT; END_VAR
        METHOD reset
            CV := 0;
        END_METHOD
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION test
VAR
    c_a : NsA.Counter;
    c_b : NsB.Counter;
END_VAR
    c_a.reset();
    c_b.reset();
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export NsA.Counter#reset(*struct(NsA.Counter))
    export NsB.Counter#reset(*struct(NsB.Counter))
    export test()
    ");
}

// -- CLASS -------------------------------------------------------------

#[rstest]
fn class_same_name_in_different_namespaces(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE NsA
    CLASS MyClass
    VAR
        value : INT;
    END_VAR
        METHOD inc
            THIS.value := THIS.value + 1;
        END_METHOD
    END_CLASS
END_NAMESPACE

NAMESPACE NsB
    CLASS MyClass
    VAR
        value : DINT;
    END_VAR
        METHOD inc
            THIS.value := THIS.value + 1;
        END_METHOD
    END_CLASS
END_NAMESPACE

FUNCTION test
VAR
    a : NsA.MyClass;
    b : NsB.MyClass;
END_VAR
    a.inc();
    b.inc();
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export NsA.MyClass#inc(*struct(NsA.MyClass))
    export NsB.MyClass#inc(*struct(NsB.MyClass))
    export test()
    ");
}

// PROGRAMs cannot be declared inside a NAMESPACE (the grammar emits
// `ERR_program_not_allowed_in_namespace`), so namespace-collision
// for programs isn't a meaningful test case at the source level.
// The qualified-naming pipeline still applies to them via
// `qualified_pou_ident`, but the input shape can't trigger it.

// -- ANY_* mangling combined with namespace qualification --------------

#[rstest]
fn any_function_mangling_keeps_namespace(mut with_db: RootDatabase) {
    // Generic function in a namespace: monomorphized exports should
    // include both the namespace and the per-T suffix.
    let source = r#"
NAMESPACE NsA
    FUNCTION abs : ANY_NUM
    VAR_INPUT IN : INTO(abs); END_VAR
    {#if IN is REAL}
        {extern 'math' 'abs_f32' (params IN) (result abs)}
    {#elif IN is LREAL}
        {extern 'math' 'abs_f64' (params IN) (result abs)}
    {#endif}
    END_FUNCTION
END_NAMESPACE

FUNCTION test
VAR r : REAL; END_VAR
    r := NsA.abs(IN := REAL#1.5);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test()
    import math.abs_f32.REAL(Real) -> Real [from NsA.abs]
    ");
}

#[rstest]
fn generic_fb_mangling_keeps_namespace(mut with_db: RootDatabase) {
    // Generic FB in a namespace: monomorphized struct/body names
    // include both the namespace and the per-T suffix.
    let source = r#"
NAMESPACE NsA
    FUNCTION_BLOCK Counter
    VAR_INPUT  PV : ANY_INT; END_VAR
    VAR_OUTPUT CV : INTO(PV); END_VAR
        CV := PV;
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION test
VAR c : NsA.Counter<INT>; END_VAR
    c(PV := 10);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export NsA.Counter$INT$__body__(*struct(NsA.Counter$INT))
    export test()
    ");
}
