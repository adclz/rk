use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_start_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := O TO 10 DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:14 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := O TO 10 DO
       |              |
       |              `-- expected 'INT', got 'BOOL'
       |              |
       |              `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn invalid_stop_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := 10 TO O DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:8:20 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := 10 TO O DO
       |                    |
       |                    `-- can't compare 'INT' with 'BOOL'
       |                    |
       |                    `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn invalid_step_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := 0 TO 10 BY O DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:8:25 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := 0 TO 10 BY O DO
       |                         |
       |                         `-- can't compare 'INT' with 'BOOL'
       |                         |
       |                         `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn while_condition_is_not_a_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    WHILE I DO

    END_WHILE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     WHILE I DO
       |           |
       |           `-- expected 'BOOL', got 'INT'
    ---'
    ");
}

#[rstest]
fn repeat_condition_is_not_a_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    REPEAT O := TRUE;
        UNTIL I
    END_REPEAT;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:15 ]
       |
     9 |         UNTIL I
       |               |
       |               `-- expected 'BOOL', got 'INT'
    ---'
    ");
}

/// IEC's grammar says `control_variable ::= identifier` — a struct field, an
/// array element, a dereference or a bit access cannot be a FOR counter, and
/// other toolchains rejects them too. These used to pass `rk check` and then die in
/// `rk compile` with an unlocated "unsupported" error: check said one thing
/// and compile another.
#[rstest]
#[case::struct_field("r.i", "r : Rec;")]
#[case::array_element("a[0]", "a : ARRAY[0..2] OF INT;")]
// (a bit access, `w.0`, is refused one layer earlier: the FOR grammar
// does not admit the token sequence at all — see the case below)
fn a_path_cannot_be_a_for_control_variable(
    mut with_db: RootDatabase,
    #[case] control: &str,
    #[case] decl: &str,
) {
    let source = format!(
        r#"
        TYPE Rec : STRUCT i : INT; END_STRUCT; END_TYPE
        FUNCTION f : INT
        VAR {decl} total : INT; END_VAR
            FOR {control} := 1 TO 5 DO
                total := total + 1;
            END_FOR;
            f := total;
        END_FUNCTION
    "#
    );
    let rendered = test_diagnostics(&mut with_db, &[&source]);
    assert!(
        rendered.contains("E1005"),
        "`FOR {control}` must be rejected with E1005, got:\n{rendered}"
    );
}

/// A bit access as a control is stopped by the GRAMMAR — the FOR rule never
/// admits it — which is even earlier than E1005. Pinned so a grammar change
/// that starts accepting it does not silently fall through to codegen.
#[rstest]
fn a_bit_access_control_variable_is_a_syntax_error(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR w : WORD; total : INT; END_VAR
            FOR w.0 := 1 TO 5 DO
                total := total + 1;
            END_FOR;
            f := total;
        END_FUNCTION
    "#;
    let rendered = test_diagnostics(&mut with_db, &[source]);
    assert!(
        rendered.contains("Error"),
        "`FOR w.0` must not pass the front end, got:\n{rendered}"
    );
}

/// ...but a bare identifier is fine WHEREVER it lives — an FB or PROGRAM
/// member is ordinary other toolchains practice. The restriction is on the syntax, not
/// on the variable's home.
#[rstest]
fn an_fb_member_can_be_a_for_control_variable(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
        VAR i : INT; total : INT; END_VAR
            FOR i := 1 TO 5 DO
                total := total + i;
            END_FOR;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
