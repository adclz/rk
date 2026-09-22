use crate::tests::codegen::{run, with_db};
use rstest::*;

/// A function with a return type and no body returns the return slot's
/// default.
#[rstest]
fn empty_function_with_return_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "f", ());
    assert_eq!(result, 0);
}

/// ...and one with no return type at all is a bare void function.
#[rstest]
fn empty_function_without_return_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f
        VAR_INPUT
            x : INT;
        END_VAR
        END_FUNCTION
    "#;
    let _: () = run(&mut with_db, source, "f", 1);
}

/// A body of nothing but comments is still an empty body.
///
/// (A lone `;` is a different question: the grammar rejects it outright as
/// "Unexpected token(s): ';'", since IEC spells a statement list as
/// `{statement ';'}` — the semicolon terminates a statement rather than being
/// one. That is a grammar decision, not a codegen gap, and it fails as a clean
/// diagnostic rather than as bad wasm.)
#[rstest]
fn function_body_of_only_comments(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
            (* nothing to do yet *)
            // still nothing
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "f", ());
    assert_eq!(result, 0);
}

/// A stub FB: declared, instantiated and invoked, but with nothing to run.
#[rstest]
fn empty_function_block(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Stub
        VAR
            x : INT;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            s : Stub;
        END_VAR
            s();
            run := s.x;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 0);
}

/// An FB with no variables *and* no body — the most degenerate case.
#[rstest]
fn empty_function_block_without_variables(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Stub
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            s : Stub;
        END_VAR
            s();
            run := 1;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 1);
}

/// An empty method — the shape a base class uses when it exists only to be
/// overridden.
#[rstest]
fn empty_method_on_a_class(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS C
        VAR
            x : INT;
        END_VAR
            METHOD PUBLIC noop
            END_METHOD

            METHOD PUBLIC get : INT
                get := 7;
            END_METHOD
        END_CLASS

        FUNCTION run : INT
        VAR
            c : C;
        END_VAR
            c.noop();
            run := c.get();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 7);
}

/// An empty method with a return type still yields its default.
#[rstest]
fn empty_method_with_return_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
            METHOD PUBLIC get : INT
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            f : FB;
        END_VAR
            run := f.get();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 0);
}

/// An empty CLASS with nothing but a field, instantiated but never called.
#[rstest]
fn empty_class(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS C
        VAR
            x : INT := 3;
        END_VAR
        END_CLASS

        FUNCTION run : INT
        VAR
            c : C;
        END_VAR
            run := 5;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 5);
}
