//! `THIS` in inherited code is the instance the code runs on.
//!
//! Nothing is looked up at run time: a method or body a derived instance
//! reaches is emitted as a copy on that instance type, where `THIS.m()` is
//! the override. A direct call already reached such a copy; these are the
//! other ways in, which reached the base's own copy instead: `SUPER()` and
//! `SUPER.m()`, a call through an interface, a method with an interface
//! parameter, and a receiver that is an array element or a dereference.

use crate::tests::codegen::{run, with_db};
use rstest::*;

/// Through an interface, an inherited method runs as the implementer's copy:
/// the override wins there, as in a direct call, and an ABSTRACT base's
/// template method reaches the implementation.
#[rstest]
fn an_interface_call_runs_the_implementers_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IShow METHOD Show : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Base IMPLEMENTS IShow
            METHOD PUBLIC Hook : INT Hook := 1; END_METHOD
            METHOD PUBLIC Show : INT Show := THIS.Hook(); END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Derived EXTENDS Base IMPLEMENTS IShow
            METHOD PUBLIC OVERRIDE Hook : INT Hook := 2; END_METHOD
        END_FUNCTION_BLOCK

        INTERFACE IShape METHOD Describe : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK ABSTRACT Shape IMPLEMENTS IShape
            METHOD PUBLIC ABSTRACT Area : INT END_METHOD
            METHOD PUBLIC Describe : INT Describe := 1000 + THIS.Area(); END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Square EXTENDS Shape IMPLEMENTS IShape
            METHOD PUBLIC OVERRIDE Area : INT Area := 16; END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION Ask : INT VAR_IN_OUT s : IShow; END_VAR Ask := s.Show(); END_FUNCTION
        FUNCTION Tell : INT VAR_IN_OUT s : IShape; END_VAR Tell := s.Describe(); END_FUNCTION

        FUNCTION test : INT
        VAR d : Derived; q : Square; END_VAR
            test := Ask(s := d) * 10000 + Tell(s := q);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 21016,
        "Derived.Hook (2), then 1000 + Square.Area (16)"
    );
}

/// `THIS.inner.m()` calls the member's method on the member; only
/// `THIS.m()` runs on the current instance.
#[rstest]
fn a_member_method_through_this_runs_on_the_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR_OUTPUT n : INT; END_VAR
            METHOD PUBLIC Bump : INT n := n + 1; Bump := n; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Outer
        VAR inner : Inner; arr : ARRAY[0..1] OF Inner; END_VAR
            METHOD PUBLIC Bump : INT Bump := 100; END_METHOD
            METHOD PUBLIC Go : INT Go := THIS.inner.Bump(); END_METHOD
            METHOD PUBLIC GoDeref : INT GoDeref := THIS^.inner.Bump(); END_METHOD
            METHOD PUBLIC GoArr : INT GoArr := THIS.arr[1].Bump(); END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR o : Outer; END_VAR
            test := o.Go() * 100 + o.GoDeref() * 10 + o.GoArr();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 121,
        "inner counts 1, 2; arr[1] counts 1; never Outer.Bump"
    );
}

/// An array element or a dereference as the receiver runs the instance's
/// copy, as a plain variable does, and binds an interface parameter.
#[rstest]
fn indexed_and_dereferenced_receivers_run_the_instances_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICall METHOD Call : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Base IMPLEMENTS ICall
            METHOD PUBLIC Hook : INT Hook := 1; END_METHOD
            METHOD PUBLIC Call : INT Call := THIS.Hook(); END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Derived EXTENDS Base IMPLEMENTS ICall
            METHOD PUBLIC OVERRIDE Hook : INT Hook := 2; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION Ask : INT VAR_IN_OUT s : ICall; END_VAR Ask := s.Call(); END_FUNCTION

        FUNCTION test : INT
        VAR d : Derived; arr : ARRAY[0..1] OF Derived; r : REF_TO Derived; i : INT := 1; END_VAR
            r := REF(d);
            test := arr[i].Call() * 1000 + r^.Call() * 100
                + Ask(s := arr[1]) * 10 + Ask(s := r^);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 2222, "Derived.Hook every time");
}
