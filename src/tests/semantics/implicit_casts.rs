use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Implicit casts according to IEC 61131-3 standard
///
/// See 6.6.1.6 Data type conversion

#[rstest]
#[case("TRUE")]
#[case("FALSE")]
#[case("BOOL#TRUE")]
#[case("BOOL#FALSE")]
fn bool_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: BYTE := {value};
        test2: WORD := {value};
        test3: DWORD := {value};
        test4: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("BYTE#0")]
fn byte_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: BYTE := {value};
        test2: WORD := {value};
        test3: DWORD := {value};
        test4: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("WORD#0")]
fn word_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: WORD := {value};
        test2: DWORD := {value};
        test3: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("DWORD#0")]
fn dword_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DWORD := {value};
        test2: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("LWORD#0")]
fn lword_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: LWORD := {value};

    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("127")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("SINT#0")]
fn sint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: SINT := {value};
        test2: INT := {value};
        test3: DINT := {value};
        test4: LINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("INT#0")]
fn int_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := {value};
        test2: DINT := {value};
        test3: LINT := {value};
        test4: REAL := {value};
        test5: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("DINT#0")]
fn dint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: DINT := {value};
        test3: LINT := {value};
        // test4: REAL := {value}; // no real (see table in standard)
        test5: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0.0")]
#[case("REAL#0.0")]
fn real_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: REAL := {value};
        test3: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("USINT#0")]
fn usint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: USINT := {value};
        test2: UINT := {value};
        test3: UDINT := {value};
        test4: ULINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("UINT#0")]
fn uint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test2: UINT := {value};
        test3: UDINT := {value};
        test4: ULINT := {value};
        test5: REAL := {value};
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("0")]
#[case("255")]
#[case("2#0101")]
#[case("8#75")]
#[case("16#A")]
#[case("UDINT#0")]
fn udint_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test3: UDINT := {value};
        test4: ULINT := {value};
        // test5: REAL := {value}; (same as sint)
        test6: LREAL := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("TIME#0s")]
fn time_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
        test2: LTIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("DT#2000-01-01-00:00:00.0")]
fn dt_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DT := {value};
        test2: LDT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("TOD#00:00:00.0")]
fn tod_implicit_cast(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := {value};
        test2: LTOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// --- binary operators widen commutatively (IEC 6.6.1.6): the result is the
// common wider type of BOTH operands, independent of operand order. The old
// left-anchored rule accepted `r * i` but rejected `i * r`. ---

#[rstest]
#[case("r * i")]
#[case("i * r")]
#[case("r + i")]
#[case("i + r")]
#[case("r - i")]
#[case("i - r")]
fn binary_operator_widening_is_commutative(mut with_db: RootDatabase, #[case] expr: &str) {
    let source = format!(
        r#"
FUNCTION f : REAL
VAR_INPUT i : INT; r : REAL; END_VAR
VAR out : REAL; END_VAR
    out := {expr};
END_FUNCTION"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("d + i")]
#[case("i + d")]
fn integer_widening_is_commutative(mut with_db: RootDatabase, #[case] expr: &str) {
    // DINT + INT widens to DINT in both orders (and DINT does NOT implicitly
    // fit REAL, so the target must be DINT).
    let source = format!(
        r#"
FUNCTION f : DINT
VAR_INPUT i : INT; d : DINT; END_VAR
    f := {expr};
END_FUNCTION"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("i < r")]
#[case("r < i")]
#[case("i = d")]
#[case("d = i")]
fn comparison_widening_is_commutative(mut with_db: RootDatabase, #[case] expr: &str) {
    let source = format!(
        r#"
FUNCTION f : BOOL
VAR_INPUT i : INT; r : REAL; d : DINT; END_VAR
    f := {expr};
END_FUNCTION"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
fn binary_operator_incompatible_types_still_rejected(mut with_db: RootDatabase) {
    // No common widening between BOOL and REAL in either order.
    let source = r#"
FUNCTION f : REAL
VAR_INPUT b : BOOL; r : REAL; END_VAR
    f := b + r;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:4:10 ]
       |
     3 | VAR_INPUT b : BOOL; r : REAL; END_VAR
       |           |
       |           `-- type is declared by variable 'b' here
     4 |     f := b + r;
       |          ^^|^^
       |            `---- operator '+' cannot be applied to type 'BOOL'
    ---'
    [E0303] Error: type mismatch
       ,-[ file:///test0.st:4:14 ]
       |
     3 | VAR_INPUT b : BOOL; r : REAL; END_VAR
       |           |
       |           `-- type is declared by variable 'b' here
     4 |     f := b + r;
       |              |
       |              `-- can not add 'BOOL' with 'REAL'
    ---'
    ");
}

#[rstest]
fn narrow_result_assignment_still_rejected(mut with_db: RootDatabase) {
    // The widened result (REAL) must not silently narrow back into an INT
    // target: the expression is fine, the assignment is not.
    let source = r#"
FUNCTION f : INT
VAR_INPUT i : INT; r : REAL; END_VAR
    f := i * r;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:10 ]
       |
     2 | FUNCTION f : INT
       |          |
       |          `-- FUNCTION 'f' is defined here, with return type 'INT'
       |
     4 |     f := i * r;
       |          ^^|^^
       |            `---- expected 'INT', got 'REAL'
       |            |
       |            `---- consider explicitly casting with 'REAL_TO_INT(i * r)'
       |
       | Help: insert explicit cast 'REAL_TO_INT(i * r)'
    ---'
    ");
}

/// Binary operators are commutative for coercion: operands are compatible iff
/// either widens to the other, and the result is their join in the widening
/// lattice (`ElementarySpec::wider`). Previously the result type was taken from
/// the LEFT operand unconditionally, so `REAL * INT` compiled while the
/// equivalent `INT * REAL` was rejected — valid code failing on operand order.
#[rstest]
#[case("r * i")]
#[case("i * r")]
#[case("r + i")]
#[case("i + r")]
#[case("r - i")]
#[case("i - r")]
#[case("r / i")]
#[case("i / r")]
fn binary_operator_operand_order_is_symmetric(mut with_db: RootDatabase, #[case] expr: &str) {
    let source = format!(
        r#"
FUNCTION fn1 : REAL
VAR_INPUT i : INT; r : REAL; END_VAR
    fn1 := {expr};
END_FUNCTION
"#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// The same symmetry for comparisons, which yield BOOL regardless of operand
/// widths.
#[rstest]
#[case("i < r")]
#[case("r < i")]
#[case("i = r")]
#[case("r = i")]
#[case("i >= d")]
#[case("d >= i")]
fn comparison_operand_order_is_symmetric(mut with_db: RootDatabase, #[case] expr: &str) {
    let source = format!(
        r#"
FUNCTION fn1 : BOOL
VAR_INPUT i : INT; r : REAL; d : DINT; END_VAR
    fn1 := {expr};
END_FUNCTION
"#
    );
    assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @"");
}

/// Symmetry must not weaken checking: a pair with no common widening stays an
/// error in BOTH orders.
#[rstest]
fn bool_plus_real_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : REAL
VAR_INPUT b : BOOL; r : REAL; END_VAR
    fn1 := b + r;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:4:12 ]
       |
     3 | VAR_INPUT b : BOOL; r : REAL; END_VAR
       |           |
       |           `-- type is declared by variable 'b' here
     4 |     fn1 := b + r;
       |            ^^|^^
       |              `---- operator '+' cannot be applied to type 'BOOL'
    ---'
    [E0303] Error: type mismatch
       ,-[ file:///test0.st:4:16 ]
       |
     3 | VAR_INPUT b : BOOL; r : REAL; END_VAR
       |           |
       |           `-- type is declared by variable 'b' here
     4 |     fn1 := b + r;
       |                |
       |                `-- can not add 'BOOL' with 'REAL'
    ---'
    ");
}

#[rstest]
fn real_plus_bool_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : REAL
VAR_INPUT b : BOOL; r : REAL; END_VAR
    fn1 := r + b;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0303] Error: type mismatch
       ,-[ file:///test0.st:4:16 ]
       |
     3 | VAR_INPUT b : BOOL; r : REAL; END_VAR
       |                     |
       |                     `-- type is declared by variable 'r' here
     4 |     fn1 := r + b;
       |                |
       |                `-- can not add 'REAL' with 'BOOL'
    ---'
    ");
}

/// The action's title promises a call; it used to carry an empty edit, so
/// applying it changed nothing and left the reader to write it out.
#[rstest]
fn the_explicit_cast_fix_writes_the_call(mut with_db: RootDatabase) {
    use auto_lsp::default::db::BaseDatabase;

    let source = r#"
FUNCTION f : INT
VAR
    a : INT;
    b : REAL;
END_VAR
    a := b;
END_FUNCTION
"#;
    crate::tests::utils::add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let written: Vec<String> = hir::check::diagnostics_for_file(&with_db, file)
        .iter()
        .flat_map(|d| d.fixes().to_vec())
        .flat_map(|fix| {
            let changes = fix.edit.and_then(|edit| edit.changes).unwrap_or_default();
            changes
                .into_values()
                .flatten()
                .map(|edit| edit.new_text)
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(written, ["REAL_TO_INT(b)"]);
}
