use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn function_as_var_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: fn;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0215] Error: invalid type
       ,-[ file:///test0.st:8:15 ]
       |
     8 |         test: fn;
       |               ^|
       |                `-- 'fn' is a function and cannot be used as a variable or data type
    ---'
    ");
}

// IEC 61131-3: RETAIN/NON_RETAIN may qualify VAR_INPUT, VAR_OUTPUT, and VAR of
// function blocks and programs — but never VAR_IN_OUT (a by-reference binding
// to the caller's storage has no state of its own to retain). The grammar
// enforces this structurally: `in_out_decls` has no retain field.
#[rstest]
fn invalid_var_in_out_retain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_IN_OUT RETAIN
        io: INT;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:3:16 ]
       |
     3 |     VAR_IN_OUT RETAIN
       |                ^^^|^^
       |                   `---- Unexpected token(s): 'RETAIN'
    ---'
    ");
}

// The legal placements from the same clause: RETAIN on VAR_INPUT, VAR_OUTPUT,
// and VAR of an FB and a PROGRAM all parse and type-check clean.
#[rstest]
fn valid_retain_qualifier_placements(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_INPUT RETAIN
        a: INT;
    END_VAR
    VAR_OUTPUT NON_RETAIN
        b: INT;
    END_VAR
    VAR RETAIN
        c: INT;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM prog1
    VAR RETAIN
        d: INT;
    END_VAR
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// E0235: RETAIN/NON_RETAIN require instance storage — meaningless on a
// stateless FUNCTION. The grammar accepts the qualifier on VAR_INPUT /
// VAR_OUTPUT sections (it is legal there for FBs/programs), so this is a
// semantic check.
#[rstest]
fn invalid_function_retain_qualifiers(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_INPUT RETAIN
        a: INT;
    END_VAR
    VAR_OUTPUT NON_RETAIN
        b: INT;
    END_VAR
    fn := a;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0235] Error: invalid retentive qualifier
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         a: INT;
       |         ^^^|^^
       |            `---- 'a' cannot be RETAIN: a FUNCTION is stateless
       |
       | Note: retentive behavior requires instance storage; only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL) can be RETAIN/NON_RETAIN
    ---'
    [E0235] Error: invalid retentive qualifier
       ,-[ file:///test0.st:7:9 ]
       |
     7 |         b: INT;
       |         ^^^|^^
       |            `---- 'b' cannot be NON_RETAIN: a FUNCTION is stateless
       |
       | Note: retentive behavior requires instance storage; only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL) can be RETAIN/NON_RETAIN
    ---'
    ");
}

// E0235 in a METHOD — methods are stateless like functions.
#[rstest]
fn invalid_method_retain_qualifier(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    METHOD m : INT
        VAR_INPUT RETAIN
            x: INT;
        END_VAR
        m := x;
    END_METHOD
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0235] Error: invalid retentive qualifier
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             x: INT;
       |             ^^^|^^
       |                `---- 'x' cannot be RETAIN: a METHOD is stateless
       |
       | Note: retentive behavior requires instance storage; only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL) can be RETAIN/NON_RETAIN
    ---'
    ");
}

// A `VAR RETAIN` SECTION in a FUNCTION is rejected by the GRAMMAR itself —
// `retain_var_decls` is not among the stateless POUs' section rules, so this
// is a syntax error rather than E0235 (context-independent restrictions live
// in the grammar; the qualifier-on-VAR_INPUT/VAR_OUTPUT case above is the
// context-dependent one).
#[rstest]
fn function_var_retain_section_is_syntax_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR RETAIN
        c: INT;
    END_VAR
    fn := 0;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:3:9 ]
       |
     3 | ,->     VAR RETAIN
     4 | |->         c: INT;
       | |
       | `--------------------- Unexpected token(s): 'RETAIN : INT ;'
    ---'
    ");
}

// CLASS variables have instance storage — RETAIN is legal there.
#[rstest]
fn valid_class_retain_qualifier(mut with_db: RootDatabase) {
    let source = r#"
CLASS c1
    VAR RETAIN
        state: INT;
    END_VAR
END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A variable named like the enclosing callable IS its return value, in any
/// case — declaring it is declaring the return value a second time.
///
/// This was only a WARNING (L0315) before, and the body then bound to the
/// local at the local's type while the signature promised the return type's:
/// `FUNCTION Wide : INT` with `Wide : LINT` emitted invalid wasm at exit 0.
/// A procedural METHOD has no return value, so its name stays free.
#[rstest]
fn variable_named_like_its_callable_is_the_return_value(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            fn1 : LINT;
        END_VAR
            fn1 := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0117] Error: duplicate definitions
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             fn1 : LINT;
       |             ^|^
       |              `--- variable 'fn1' is the FUNCTION's return value
    ---'
    ");
}
