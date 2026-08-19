use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn not_a_callable_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0229] Error: semantic violation
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     test();
       |     ^^|^
       |       `--- 'INT' is not a callable type
    ---'
    ");
}

#[rstest]
fn uninstantied_fb_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    fb2();
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0229] Error: semantic violation
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     fb2();
       |     ^|^
       |      `--- 'fb2' is not a callable type
       |
       | Note: to call a FUNCTION_BLOCK, you need to instantiate it first.
    ---'
    ");
}

// should not emit any errors
#[rstest]
fn array_of_fb_instances(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
    VAR
        instances: ARRAY[0..1] OF fb1;
    END_VAR

    instances[0]();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn unknown_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
     VAR_INPUT
        u: BOOL;
     END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown := TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0208] Error: function call parameter mismatch
        ,-[ file:///test0.st:10:9 ]
        |
     10 |         unknown := TRUE
        |         ^^^|^^^
        |            `----- unknown input parameter 'unknown'
    ----'
    [E0233] Error: missing required parameter
       ,-[ file:///test0.st:9:5 ]
       |
     4 |         u: BOOL;
       |         ^^^|^^^
       |            `----- parameter 'u' declared here
       |
     9 |     fn(
       |     ^|
       |      `-- call to 'fn' is missing 1 required parameter: 'u'
       |
       | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ---'
    ");
}

#[rstest]
fn unknown_output_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown => TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0205] Error: function call parameter mismatch
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     fn(
       |     ^|
       |      `-- 'fn' expects 0 parameters, but got 1
    ---'
    [E0209] Error: function call parameter mismatch
       ,-[ file:///test0.st:7:9 ]
       |
     7 |         unknown => TRUE
       |         ^^^|^^^
       |            `----- unknown output parameter 'unknown'
    ---'
    ");
}

#[rstest]
fn type_check_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_INPUT
    param1: LINT;
    param2: LREAL;
  END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        param1 := 5.5,
        param2 := TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:11:19 ]
        |
      4 |     param1: LINT;
        |     ^^^|^^
        |        `---- type is declared by variable 'param1' here
        |
     11 |         param1 := 5.5,
        |                   ^|^
        |                    `--- cannot infer '<float>' to 'LINT': invalid LINT literal
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:12:19 ]
        |
      5 |     param2: LREAL;
        |     ^^^|^^
        |        `---- type is declared by variable 'param2' here
        |
     12 |         param2 := TRUE
        |                   ^^|^
        |                     `--- expected 'LREAL', got 'BOOL'
    ----'
    ");
}

#[rstest]
fn type_check_output_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_INPUT
    param1: INT;
    param2: REAL;
  END_VAR

  VAR_OUTPUT
    param3: INT;
  END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        variable1: BOOL;
    END_VAR

    fn(
        param1 := TRUE,
        param2 := TRUE,
        param3 => variable1
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:19:19 ]
        |
      4 |     param1: INT;
        |     ^^^|^^
        |        `---- type is declared by variable 'param1' here
        |
     19 |         param1 := TRUE,
        |                   ^^|^
        |                     `--- expected 'INT', got 'BOOL'
        |                     |
        |                     `--- consider explicitly casting with 'BOOL_TO_INT(TRUE)'
        |
        | Help: insert explicit cast 'BOOL_TO_INT(TRUE)'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:19 ]
        |
      5 |     param2: REAL;
        |     ^^^|^^
        |        `---- type is declared by variable 'param2' here
        |
     20 |         param2 := TRUE,
        |                   ^^|^
        |                     `--- expected 'REAL', got 'BOOL'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:21:19 ]
        |
     15 |         variable1: BOOL;
        |         ^^^^|^^^^
        |             `------ type is declared by variable 'variable1' here
        |
     21 |         param3 => variable1
        |                   ^^^^|^^^^
        |                       `------ expected 'INT', got 'BOOL'
        |                       |
        |                       `------ consider explicitly casting with 'BOOL_TO_INT(variable1)'
        |
        | Help: insert explicit cast 'BOOL_TO_INT(variable1)'
    ----'
    ");
}

// valid if the order of parameters is correct
#[rstest]
fn mixing_non_formal_and_formal_parameters(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1: INT;
        param2: REAL;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 := 0, 1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn too_many_parameters(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR_INPUT
		param1: INT;
		param2: REAL;
	END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

	fn(0, 1.5, 5);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0205] Error: function call parameter mismatch
        ,-[ file:///test0.st:11:2 ]
        |
     11 |     fn(0, 1.5, 5);
        |     ^|
        |      `-- 'fn' expects 2 parameters, but got 3
    ----'
    [E0206] Error: function call parameter mismatch
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     fn(0, 1.5, 5);
        |                |
        |                `-- no parameter at index '2'
    ----'
    ");
}

#[rstest]
fn duplicate_input_parameter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1: INT;
        param2: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 := 0, param1 := 1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0108] Error: duplicate definitions
        ,-[ file:///test0.st:11:21 ]
        |
     11 |     fn(param1 := 0, param1 := 1);
        |        ^^^^^|^^^^^  ^^^^^|^^^^^
        |             `-------------------- previously defined here
        |                          |
        |                          `------- duplicate parameter 'param1' found
    ----'
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |         param2: INT;
        |         ^^^^^|^^^^^
        |              `------- parameter 'param2' declared here
        |
     11 |     fn(param1 := 0, param1 := 1);
        |     ^|
        |      `-- call to 'fn' is missing 1 required parameter: 'param2'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

#[rstest]
fn duplicate_output_parameter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_OUTPUT
        param1: INT;
        param2: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        a1: INT;
        a2: INT;
    END_VAR

    fn(param1 => a1, param1 => a2);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0108] Error: duplicate definitions
        ,-[ file:///test0.st:15:22 ]
        |
     15 |     fn(param1 => a1, param1 => a2);
        |        ^^^^^^|^^^^^  ^^^^^^|^^^^^
        |              `--------------------- previously defined here
        |                            |
        |                            `------- duplicate parameter 'param1' found
    ----'
    ");
}

#[rstest]
fn output_assignment_is_not_a_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE b1 : INT
END_TYPE

FUNCTION fn
    VAR_OUTPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 => b1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:13:18 ]
        |
     13 |     fn(param1 => b1);
        |                  ^|
        |                   `-- cannot use direct type 'b1' here
    ----'
    ");
}

#[rstest]
fn output_assignment_is_an_input_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_OUTPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR_INPUT
        b1: INT;
    END_VAR

    fn(param1 => b1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn missing_function_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_INPUT
        a: INT;
        b: REAL;
    END_VAR
    fn := 0;
END_FUNCTION

FUNCTION_BLOCK fb1
    fn();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:11:5 ]
        |
      4 |         a: INT;
        |         ^^^|^^
        |            `---- parameter 'a' declared here
      5 |         b: REAL;
        |         ^^^|^^^
        |            `----- parameter 'b' declared here
        |
     11 |     fn();
        |     ^|
        |      `-- call to 'fn' is missing 2 required parameters: 'a', 'b'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

#[rstest]
fn missing_function_var_input_partial(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_INPUT
        a: INT;
        b: REAL;
    END_VAR
    fn := 0;
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(a := 1);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |         b: REAL;
        |         ^^^|^^^
        |            `----- parameter 'b' declared here
        |
     11 |     fn(a := 1);
        |     ^|
        |      `-- call to 'fn' is missing 1 required parameter: 'b'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

// FUNCTION VAR_INPUT with a scalar default value can be omitted at the call site.
#[rstest]
fn function_var_input_with_default(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_INPUT
        a: INT := 42;
        b: REAL := 1.5;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR x: INT; END_VAR
    x := fn();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// Mixing: one VAR_INPUT has a default, the other does not.
// The one without a default is required.
#[rstest]
fn function_var_input_partial_default(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_INPUT
        a: INT;
        b: REAL := 1.5;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR x: INT; END_VAR
    x := fn();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:12:10 ]
        |
      4 |         a: INT;
        |         ^^^|^^
        |            `---- parameter 'a' declared here
        |
     12 |     x := fn();
        |          ^|
        |           `-- call to 'fn' is missing 1 required parameter: 'a'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

// A compound (struct-literal) default value cannot yet be materialized at the
// call site, so the param remains required.
#[rstest]
fn function_var_input_with_struct_default_still_required(mut with_db: RootDatabase) {
    let source = r#"
TYPE point :
    STRUCT
        x: INT;
        y: INT;
    END_STRUCT
END_TYPE

FUNCTION fn : INT
    VAR_INPUT
        p: point := (x := 1, y := 2);
    END_VAR
    fn := p.x;
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR r: INT; END_VAR
    r := fn();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:18:10 ]
        |
     11 |         p: point := (x := 1, y := 2);
        |         ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
        |                       `--------------- parameter 'p' declared here
        |
     18 |     r := fn();
        |          ^|
        |           `-- call to 'fn' is missing 1 required parameter: 'p'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

// FUNCTION_BLOCK: omitting VAR_INPUT at a call site is OK — the FB instance
// retains the value across calls.
#[rstest]
fn function_block_var_input_omitted(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK ramp
    VAR_INPUT
        target: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        r: ramp;
    END_VAR
    r();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// VAR_IN_OUT is always required, regardless of POU kind, because it must bind
// to a caller-side l-value.
#[rstest]
fn missing_function_var_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_IN_OUT
        a: INT;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    fn();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:10:5 ]
        |
      4 |         a: INT;
        |         ^^^|^^
        |            `---- parameter 'a' declared here
        |
     10 |     fn();
        |     ^|
        |      `-- call to 'fn' is missing 1 required parameter: 'a'
        |
        | Note: VAR_IN_OUT parameters bind to caller-side l-values and must always be supplied
    ----'
    ");
}

#[rstest]
fn missing_fb_var_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK driver
    VAR_IN_OUT
        target: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        d: driver;
    END_VAR
    d();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:12:5 ]
        |
      4 |         target: INT;
        |         ^^^^^|^^^^^
        |              `------- parameter 'target' declared here
        |
     12 |     d();
        |     |
        |     `-- call to 'driver' is missing 1 required parameter: 'target'
        |
        | Note: VAR_IN_OUT parameters bind to caller-side l-values and must always be supplied
    ----'
    ");
}

// METHODs behave like FUNCTIONs for the required-VAR_INPUT rule.
#[rstest]
fn missing_method_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    METHOD m : INT
        VAR_INPUT
            a: INT;
        END_VAR
        m := a;
    END_METHOD

    THIS.m();
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0233] Error: missing required parameter
        ,-[ file:///test0.st:10:5 ]
        |
      5 |             a: INT;
        |             ^^^|^^
        |                `---- parameter 'a' declared here
        |
     10 |     THIS.m();
        |     ^^^|^^
        |        `---- call to 'm' is missing 1 required parameter: 'a'
        |
        | Note: VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value
    ----'
    ");
}

// E0234: a VAR_IN_OUT argument must be an l-value — it binds the callee to the
// caller's storage by reference, so a literal has no address to bind.
#[rstest]
fn invalid_function_var_in_out_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_IN_OUT
        a: INT;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(a := 5);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0234] Error: VAR_IN_OUT argument must be a variable
        ,-[ file:///test0.st:10:13 ]
        |
      4 |         a: INT;
        |         ^^^|^^
        |            `---- parameter 'a' declared here
        |
     10 |     fn(a := 5);
        |             |
        |             `-- VAR_IN_OUT parameter 'a' of 'fn' requires a variable, not a value
        |
        | Note: VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind
    ----'
    ");
}

// E0234 also fires for arithmetic expressions and call results.
#[rstest]
fn invalid_function_var_in_out_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_IN_OUT
        a: INT;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        x: INT;
    END_VAR
    fn(x + 1);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0234] Error: VAR_IN_OUT argument must be a variable
        ,-[ file:///test0.st:13:8 ]
        |
      4 |         a: INT;
        |         ^^^|^^
        |            `---- parameter 'a' declared here
        |
     13 |     fn(x + 1);
        |        ^^|^^
        |          `---- VAR_IN_OUT parameter 'a' of 'fn' requires a variable, not a value
        |
        | Note: VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind
    ----'
    ");
}

// E0234 on a FUNCTION_BLOCK inout argument.
#[rstest]
fn invalid_fb_var_in_out_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK driver
    VAR_IN_OUT
        target: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        d: driver;
    END_VAR
    d(target := 42);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0234] Error: VAR_IN_OUT argument must be a variable
        ,-[ file:///test0.st:12:17 ]
        |
      4 |         target: INT;
        |         ^^^^^|^^^^^
        |              `------- parameter 'target' declared here
        |
     12 |     d(target := 42);
        |                 ^|
        |                  `-- VAR_IN_OUT parameter 'target' of 'driver' requires a variable, not a value
        |
        | Note: VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind
    ----'
    ");
}

// Valid l-values for VAR_IN_OUT: a variable, a struct field, an array element.
#[rstest]
fn valid_var_in_out_lvalues(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT n : INT; END_STRUCT; END_TYPE

FUNCTION fn : INT
    VAR_IN_OUT
        a: INT;
    END_VAR
    fn := a;
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        x: INT;
        p: Pair;
        arr: ARRAY[0..2] OF INT;
    END_VAR
    fn(a := x);
    fn(a := p.n);
    fn(a := arr[1]);
    fn(x);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// E0236: a VAR_IN_OUT parameter cannot be bound with output syntax — `=>`
// would leave the by-reference binding unbound.
#[rstest]
fn invalid_var_in_out_arrow_binding(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn : INT
    VAR_IN_OUT
        io: INT;
    END_VAR
    fn := io;
END_FUNCTION

FUNCTION_BLOCK driver
    VAR_IN_OUT
        target: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        d: driver;
        x: INT;
        y: INT;
    END_VAR
    fn(io => x);
    d(target => y);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0236] Error: VAR_IN_OUT parameter bound with output syntax
        ,-[ file:///test0.st:21:8 ]
        |
      4 |         io: INT;
        |         ^^^|^^^
        |            `----- parameter 'io' declared here
        |
     21 |     fn(io => x);
        |        ^|
        |         `-- VAR_IN_OUT parameter 'io' of 'fn' cannot be bound with '=>'
        |
        | Note: VAR_IN_OUT is bound by reference at call entry: use io := <variable>
    ----'
    [E0236] Error: VAR_IN_OUT parameter bound with output syntax
        ,-[ file:///test0.st:22:7 ]
        |
     11 |         target: INT;
        |         ^^^^^|^^^^^
        |              `------- parameter 'target' declared here
        |
     22 |     d(target => y);
        |       ^^^|^^
        |          `---- VAR_IN_OUT parameter 'target' of 'driver' cannot be bound with '=>'
        |
        | Note: VAR_IN_OUT is bound by reference at call entry: use target := <variable>
    ----'
    ");
}

/// A partial access (`b.%X1`) is a VariableAccess syntactically but names a
/// SLICE of a variable — a slice has no address, and VAR_IN_OUT is an
/// address. This used to pass the checker; the argument then reached the
/// callee as a bit VALUE standing where a pointer belongs, and the callee's
/// writes corrupted memory near address zero.
#[rstest]
fn invalid_in_out_partial_access(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SetIt : INT
VAR_IN_OUT io : BOOL; END_VAR
    io := TRUE;
END_FUNCTION

FUNCTION_BLOCK FBSet
VAR_IN_OUT io : BOOL; END_VAR
    io := TRUE;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR b : BYTE; f : FBSet; END_VAR
    SetIt(io := b.%X1);
    f(io := b.%X1);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0234] Error: VAR_IN_OUT argument must be a variable
        ,-[ file:///test0.st:14:17 ]
        |
      3 | VAR_IN_OUT io : BOOL; END_VAR
        |            ^^^^|^^^^
        |                `------ parameter 'io' declared here
        |
     14 |     SetIt(io := b.%X1);
        |                 ^^|^^
        |                   `---- VAR_IN_OUT parameter 'io' of 'SetIt' requires a variable, not a value
        |
        | Note: VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind
    ----'
    [E0234] Error: VAR_IN_OUT argument must be a variable
        ,-[ file:///test0.st:15:13 ]
        |
      8 | VAR_IN_OUT io : BOOL; END_VAR
        |            ^^^^|^^^^
        |                `------ parameter 'io' declared here
        |
     15 |     f(io := b.%X1);
        |             ^^|^^
        |               `---- VAR_IN_OUT parameter 'io' of 'FBSet' requires a variable, not a value
        |
        | Note: VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind
    ----'
    ");
}
