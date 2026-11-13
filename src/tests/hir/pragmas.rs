// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn valid_extern_pragma_minimal(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The core interface a WASI import keeps: inputs become params, and the
/// scalar outputs in declaration order, the return type last, are the
/// fields of the record it returns. Nothing is restated in the pragma, so
/// nothing can disagree with the declaration. The interface is made up: any
/// in the `wasi` namespace keeps the core rules, and WASI's WIT decides at
/// the build.
#[rstest]
fn valid_wasi_extern_with_outputs_and_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'wasi:test/rt@0.1.0' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : REAL; status : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// VAR_IN_OUT would hand the host a pointer into caller storage with a
/// mutation contract; an extern's interface is copies only.
#[rstest]
fn invalid_extern_in_out(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'fill'}
FUNCTION fill : INT
VAR_IN_OUT buf : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR_IN_OUT buf : INT; END_VAR
       |            ^|^
       |             `--- VAR_IN_OUT 'buf' cannot cross a WASM import: an extern takes copies, not references
       |
       | Note: an extern FUNCTION receives copies of its VAR_INPUT
    ---'
    ");
}

/// An import returns a record of scalars: a STRUCT or a STRING output is
/// refused, on a WASI import too, which keeps the core rules (the interface
/// is made up), and on any other. An output or a return type that does not
/// resolve is reported where the type is written, and nowhere else.
#[rstest]
fn invalid_extern_aggregate_output(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

{extern 'wasi:test/host@0.1.0' 'point'}
FUNCTION point : INT
VAR_OUTPUT p : Pt; s : STRING; END_VAR
END_FUNCTION

{extern 'host' 'label'}
FUNCTION label : INT
VAR_OUTPUT text : STRING; u : Unknown; END_VAR
END_FUNCTION

{extern 'host' 'gone'}
FUNCTION gone : Missing
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:6:12 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |            |
       |            `-- VAR_OUTPUT 'p' cannot be a field of the import's result: only a scalar can
       |
       | Note: an {extern} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars
    ---'
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:6:20 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |                    |
       |                    `-- VAR_OUTPUT 's' cannot be a field of the import's result: only a scalar can
       |
       | Note: an {extern} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars
    ---'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:11:12 ]
        |
     11 | VAR_OUTPUT text : STRING; u : Unknown; END_VAR
        |            ^^|^
        |              `--- VAR_OUTPUT 'text' cannot be a field of the import's result: only a scalar can
        |
        | Note: an {extern} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars
    ----'
    [E0203] Error: unknown name in a namespace
        ,-[ file:///test0.st:11:31 ]
        |
     11 | VAR_OUTPUT text : STRING; u : Unknown; END_VAR
        |                               ^^^|^^^
        |                                  `----- no item found for path 'Unknown'
    ----'
    [E0203] Error: unknown name in a namespace
        ,-[ file:///test0.st:15:17 ]
        |
     15 | FUNCTION gone : Missing
        |                 ^^^|^^^
        |                    `----- no item found for path 'Missing'
    ----'
    ");
}

/// A WASI import keeps WASI's types in their core form: a STRUCT, an ARRAY
/// or an instance input would go as an address into rk's own layout, which
/// no WASI function reads, so each is refused. A `REF_TO`, where a record
/// comes back, and a STRING are not.
#[rstest]
fn invalid_wasi_extern_aggregate_input(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

{extern 'wasi:test/host@0.1.0' 'send'}
FUNCTION send : INT
VAR_INPUT
    p : Pt;
    a : ARRAY[0..3] OF BYTE;
    c : Counter;
    r : REF_TO Pt;
    s : STRING;
END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     p : Pt;
        |     |
        |     `-- VAR_INPUT 'p' cannot cross a WASI import: no WASI function reads rk's own layout
        |
        | Note: a WASI import takes scalars, STRINGs, and a REF_TO where a record comes back
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     a : ARRAY[0..3] OF BYTE;
        |     |
        |     `-- VAR_INPUT 'a' cannot cross a WASI import: no WASI function reads rk's own layout
        |
        | Note: a WASI import takes scalars, STRINGs, and a REF_TO where a record comes back
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     c : Counter;
        |     |
        |     `-- VAR_INPUT 'c' cannot cross a WASI import: no WASI function reads rk's own layout
        |
        | Note: a WASI import takes scalars, STRINGs, and a REF_TO where a record comes back
    ----'
    ");
}

/// The import IS the body: statements on an extern FUNCTION are refused.
#[rstest]
fn invalid_extern_with_body(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    test := x;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     test := x;
       |     ^^^^|^^^^
       |         `------ an extern FUNCTION has no statements
       |
       | Note: an {extern} FUNCTION is an import: the host has its body
    ---'
    ");
}

/// Only a FUNCTION lowers to an import, and a FUNCTION_BLOCK to an async
/// one; on a PROGRAM the pragma used to be silently ignored, leaving the
/// body it stood on empty.
#[rstest]
fn invalid_extern_outside_function(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'nope'}
PROGRAM Main
VAR n : INT; END_VAR
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1501] Error: extern pragma outside a FUNCTION or FUNCTION_BLOCK
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {extern 'host' 'nope'}
       | ^^^^^^^^^^^|^^^^^^^^^^
       |            `------------ an {extern} pragma cannot be placed on a PROGRAM
       |
       | Note: only a FUNCTION or a FUNCTION_BLOCK can be {extern}
    ---'
    ");
}

/// An async call is held by a FUNCTION_BLOCK's instance from one scan to
/// the next: a FUNCTION_BLOCK's `{extern}` says `async`, and a FUNCTION's
/// cannot.
#[rstest]
fn invalid_extern_async_on_the_wrong_pou(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'poll'}
FUNCTION_BLOCK Poll
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{extern async 'host' 'read'}
FUNCTION Read : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1501] Error: extern pragma outside a FUNCTION or FUNCTION_BLOCK
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {extern 'host' 'poll'}
       | ^^^^^^^^^^^|^^^^^^^^^^
       |            `------------ an {extern} pragma on a FUNCTION_BLOCK needs async
       |
       | Help: write {extern async 'module' 'name'}
       |
       | Note: a FUNCTION_BLOCK imports an async function, whose call its instance holds from one scan to the next
    ---'
    [E1501] Error: extern pragma outside a FUNCTION or FUNCTION_BLOCK
       ,-[ file:///test0.st:7:1 ]
       |
     7 | {extern async 'host' 'read'}
       | ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |               `--------------- an {extern async} pragma cannot be placed on a FUNCTION
       |
       | Help: declare a FUNCTION_BLOCK with a VAR_OUTPUT DONE : BOOL
       |
       | Note: an async call is held from one scan to the next, which only a FUNCTION_BLOCK's instance can
    ---'
    ");
}

/// An async import declares its DONE, a BOOL in VAR_OUTPUT: missing, of
/// another type or in another section, it is refused.
#[rstest]
fn invalid_async_import_without_done(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'host' 'a'}
FUNCTION_BLOCK A
VAR_INPUT x : INT; END_VAR
END_FUNCTION_BLOCK

{extern async 'host' 'b'}
FUNCTION_BLOCK B
VAR_OUTPUT done : INT; END_VAR
END_FUNCTION_BLOCK

{extern async 'host' 'c'}
FUNCTION_BLOCK C
VAR_INPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1514] Error: async import without its DONE
       ,-[ file:///test0.st:3:16 ]
       |
     3 | FUNCTION_BLOCK A
       |                |
       |                `-- 'A' has no VAR_OUTPUT DONE
       |
       | Help: declare DONE : BOOL in VAR_OUTPUT
       |
       | Note: an {extern async} FUNCTION_BLOCK's DONE is its own and no part of the function's signature: it is TRUE in the call of the block that saw the call finish, and the other VAR_OUTPUTs are the result's fields
    ---'
    [E1514] Error: async import without its DONE
       ,-[ file:///test0.st:9:12 ]
       |
     9 | VAR_OUTPUT done : INT; END_VAR
       |            ^^|^
       |              `--- 'done' is not a VAR_OUTPUT of type BOOL
       |
       | Help: declare DONE : BOOL in VAR_OUTPUT
       |
       | Note: an {extern async} FUNCTION_BLOCK's DONE is its own and no part of the function's signature: it is TRUE in the call of the block that saw the call finish, and the other VAR_OUTPUTs are the result's fields
    ---'
    [E1514] Error: async import without its DONE
        ,-[ file:///test0.st:14:11 ]
        |
     14 | VAR_INPUT DONE : BOOL; END_VAR
        |           ^^|^
        |             `--- 'DONE' is not a VAR_OUTPUT of type BOOL
        |
        | Help: declare DONE : BOOL in VAR_OUTPUT
        |
        | Note: an {extern async} FUNCTION_BLOCK's DONE is its own and no part of the function's signature: it is TRUE in the call of the block that saw the call finish, and the other VAR_OUTPUTs are the result's fields
    ----'
    ");
}

/// The host writes an async call's result into its instance at a later
/// scan: a FUNCTION's and a METHOD's variables and a VAR_TEMP are gone by
/// then, and an instance holding an async import's is refused there too. A
/// PROGRAM's and a FUNCTION_BLOCK's are not.
#[rstest]
fn invalid_async_import_in_a_frame(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
FUNCTION_BLOCK WAIT_FOR
VAR_INPUT how_long : LTIME; END_VAR
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Holder
VAR wait : WAIT_FOR; END_VAR
END_FUNCTION_BLOCK

FUNCTION Pause
VAR wait : WAIT_FOR; holder : Holder; END_VAR
END_FUNCTION

FUNCTION_BLOCK Machine
VAR_TEMP wait : WAIT_FOR; END_VAR
METHOD Stop
VAR waits : ARRAY[0..1] OF WAIT_FOR; END_VAR
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR wait : WAIT_FOR; holder : Holder; END_VAR
    wait(how_long := LT#1s);
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1515] Error: async import in a call's frame
        ,-[ file:///test0.st:13:5 ]
        |
     13 | VAR wait : WAIT_FOR; holder : Holder; END_VAR
        |     ^^|^
        |       `--- 'wait' holds an async import, which cannot live in a FUNCTION's frame
        |
        | Help: declare it in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL
        |
        | Note: the host writes an async call's result into its instance when it finishes, at a later scan, when the frame is gone
    ----'
    [E1515] Error: async import in a call's frame
        ,-[ file:///test0.st:13:22 ]
        |
     13 | VAR wait : WAIT_FOR; holder : Holder; END_VAR
        |                      ^^^|^^
        |                         `---- 'holder' holds an async import, which cannot live in a FUNCTION's frame
        |
        | Help: declare it in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL
        |
        | Note: the host writes an async call's result into its instance when it finishes, at a later scan, when the frame is gone
    ----'
    [E1515] Error: async import in a call's frame
        ,-[ file:///test0.st:17:10 ]
        |
     17 | VAR_TEMP wait : WAIT_FOR; END_VAR
        |          ^^|^
        |            `--- 'wait' holds an async import, which cannot live in VAR_TEMP
        |
        | Help: declare it in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL
        |
        | Note: the host writes an async call's result into its instance when it finishes, at a later scan, when the frame is gone
    ----'
    [E1515] Error: async import in a call's frame
        ,-[ file:///test0.st:19:5 ]
        |
     19 | VAR waits : ARRAY[0..1] OF WAIT_FOR; END_VAR
        |     ^^|^^
        |       `---- 'waits' holds an async import, which cannot live in a METHOD's frame
        |
        | Help: declare it in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL
        |
        | Note: the host writes an async call's result into its instance when it finishes, at a later scan, when the frame is gone
    ----'
    ");
}

/// An async import's body is its call: statements are refused, as on an
/// `{extern}` FUNCTION.
#[rstest]
fn invalid_async_import_with_body(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'host' 'tick'}
FUNCTION_BLOCK Tick
VAR_OUTPUT DONE : BOOL; END_VAR
    DONE := TRUE;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     DONE := TRUE;
       |     ^^^^^^|^^^^^
       |           `------- an extern FUNCTION_BLOCK has no statements
       |
       | Note: an {extern} FUNCTION_BLOCK is an import: the host has its body
    ---'
    ");
}

/// An async import's block is its call and nothing more: a METHOD, a base
/// or an INTERFACE would be code the host has not. Each is refused at the
/// METHOD's name or at the type named.
#[rstest]
fn invalid_async_import_with_members(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
VAR_INPUT extra : DINT; END_VAR
END_FUNCTION_BLOCK

INTERFACE Pollable
END_INTERFACE

{extern async 'host' 'tick'}
FUNCTION_BLOCK Tick EXTENDS Base IMPLEMENTS Pollable
VAR_OUTPUT DONE : BOOL; END_VAR
METHOD Again : BOOL
    Again := DONE;
END_METHOD
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:10:29 ]
        |
     10 | FUNCTION_BLOCK Tick EXTENDS Base IMPLEMENTS Pollable
        |                             ^^|^
        |                               `--- an extern FUNCTION_BLOCK extends nothing
        |
        | Help: extend the base in a FUNCTION_BLOCK that holds the import
        |
        | Note: an {extern} FUNCTION_BLOCK is an import: the host has its body
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:10:45 ]
        |
     10 | FUNCTION_BLOCK Tick EXTENDS Base IMPLEMENTS Pollable
        |                                             ^^^^|^^^
        |                                                 `----- an extern FUNCTION_BLOCK implements no INTERFACE
        |
        | Help: implement the INTERFACE in a FUNCTION_BLOCK that holds the import
        |
        | Note: an {extern} FUNCTION_BLOCK is an import: the host has its body
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:12:8 ]
        |
     12 | METHOD Again : BOOL
        |        ^^|^^
        |          `---- an extern FUNCTION_BLOCK has no METHOD
        |
        | Help: declare the METHOD in a FUNCTION_BLOCK that holds the import
        |
        | Note: an {extern} FUNCTION_BLOCK is an import: the host has its body
    ----'
    ");
}

/// A FUNCTION can move a block it does not hold: the caller's, handed as
/// VAR_IN_OUT, and a global's, reached through VAR_EXTERNAL. Neither
/// instance is in the FUNCTION's frame, so the call outlives the FUNCTION's
/// own call safely. Accepted, and the module they build is valid.
#[rstest]
fn valid_async_import_reached_from_a_function(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
FUNCTION_BLOCK WAIT_FOR
VAR_INPUT how_long : LTIME; END_VAR
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION Step : BOOL
VAR_IN_OUT wait : WAIT_FOR; END_VAR
    wait(how_long := LT#5ms);
    Step := wait.DONE;
END_FUNCTION

FUNCTION StepShared : BOOL
VAR_EXTERNAL shared : WAIT_FOR; END_VAR
    shared(how_long := LT#7ms);
    StepShared := shared.DONE;
END_FUNCTION

PROGRAM Main
VAR mine : WAIT_FOR; both : BOOL; END_VAR
    both := Step(wait := mine) AND StepShared();
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL shared : WAIT_FOR; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P WITH T : Main;
    END_RESOURCE
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A WASI import may be a stream's or a future's built-in, under the name
/// of the function it serves: those that return at once on a FUNCTION, and
/// a stream's write, which waits for the reader, on an `{extern async}`
/// FUNCTION_BLOCK. Accepted, and the module they build is valid.
#[rstest]
fn valid_stream_built_ins(mut with_db: RootDatabase) {
    let source = r#"
{extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
FUNCTION NEW_STREAM
VAR_OUTPUT readable : UDINT; writable : UDINT; END_VAR
END_FUNCTION

{extern 'wasi:cli/stdout@0.3.0' '[future-drop-readable-1]write-via-stream'}
FUNCTION DROP_FUTURE
VAR_INPUT future : UDINT; END_VAR
END_FUNCTION

{extern 'wasi:cli/stdout@0.3.0' '[stream-drop-writable-0]write-via-stream'}
FUNCTION DROP_WRITER
VAR_INPUT stream : UDINT; END_VAR
END_FUNCTION

{extern async 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
FUNCTION_BLOCK STREAM_WRITE
VAR_INPUT stream : UDINT; data : STRING[64]; END_VAR
VAR_OUTPUT DONE : BOOL; written : UDINT; closed : BOOL; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR reading : UDINT; writing : UDINT; write : STREAM_WRITE; END_VAR
    NEW_STREAM(readable => reading, writable => writing);
    write(stream := writing, data := 'hello');
    DROP_FUTURE(future := 3);
    DROP_WRITER(stream := 2);
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A built-in fits one kind of POU (E1516): a read or a write waits for the
/// other end, so only an `{extern async}` block imports it, and the others
/// return at once, so a FUNCTION does. An async block imports a call or a
/// stream's write, which declares the built-in's variables, and a `new`
/// declares the two ends it makes, a UDINT VAR_OUTPUT each. Outside WASI, a
/// built-in's name is no WIT name (E1512).
#[rstest]
fn invalid_stream_built_ins(mut with_db: RootDatabase) {
    let source = r#"
{extern 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
FUNCTION WRITE_NOW : UDINT
VAR_INPUT stream : UDINT; END_VAR
END_FUNCTION

{extern async 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
FUNCTION_BLOCK NEW_LATER
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{extern async 'wasi:cli/stdin@0.3.0' '[stream-read-0]read-via-stream'}
FUNCTION_BLOCK READ
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{extern async 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
FUNCTION_BLOCK WRITE_TEXT
VAR_INPUT text : STRING[64]; END_VAR
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{extern 'host' '[stream-new-0]send'}
FUNCTION NEW_HOST_STREAM : LWORD
END_FUNCTION

{extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
FUNCTION NEW_AS_ONE : LWORD
END_FUNCTION

{extern 'wasi:cli/stdout@0.3.0' '[future-new-1]write-via-stream'}
FUNCTION NEW_FUTURE
VAR_OUTPUT readable : UDINT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1516] Error: stream built-in that does not fit its declaration
       ,-[ file:///test0.st:2:33 ]
       |
     2 | {extern 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
       |                                 ^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
       |                                                  `------------------ '[stream-write-0]write-via-stream' waits for the other end
       |
       | Help: import it with {extern async} on a FUNCTION_BLOCK
       |
       | Note: a read or a write of a stream or a future waits for the other end, which only the calls of an {extern async} FUNCTION_BLOCK can
    ---'
    [E1516] Error: stream built-in that does not fit its declaration
       ,-[ file:///test0.st:7:39 ]
       |
     7 | {extern async 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
       |                                       ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^
       |                                                       `----------------- '[stream-new-0]write-via-stream' returns at once
       |
       | Help: import it with {extern} on a FUNCTION
       |
       | Note: an {extern async} FUNCTION_BLOCK imports a call that may not return at once
    ---'
    [E1516] Error: stream built-in that does not fit its declaration
        ,-[ file:///test0.st:12:38 ]
        |
     12 | {extern async 'wasi:cli/stdin@0.3.0' '[stream-read-0]read-via-stream'}
        |                                      ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^
        |                                                      `----------------- '[stream-read-0]read-via-stream' is no call and no stream write
        |
        | Note: an {extern async} FUNCTION_BLOCK imports the call of an async func or the write of a stream
    ----'
    [E1516] Error: stream built-in that does not fit its declaration
        ,-[ file:///test0.st:18:16 ]
        |
     18 | FUNCTION_BLOCK WRITE_TEXT
        |                ^^^^^|^^^^
        |                     `------ 'WRITE_TEXT' does not declare a stream write's variables
        |
        | Help: declare the VAR_INPUTs stream : UDINT and data : STRING, and the VAR_OUTPUTs written : UDINT and closed : BOOL after DONE
        |
        | Note: a stream write takes the stream's writing end, a UDINT, then the bytes, a STRING, and gives after DONE how many went, a UDINT, then whether the reader left, a BOOL
    ----'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:23:16 ]
        |
     23 | {extern 'host' '[stream-new-0]send'}
        |                ^^^^^^^^^^|^^^^^^^^^
        |                          `----------- '[stream-new-0]send' is not a WIT name
        |
        | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ----'
    [E1516] Error: stream built-in that does not fit its declaration
        ,-[ file:///test0.st:28:10 ]
        |
     28 | FUNCTION NEW_AS_ONE : LWORD
        |          ^^^^^|^^^^
        |               `------ 'NEW_AS_ONE' does not declare the two ends it makes
        |
        | Help: declare the VAR_OUTPUTs readable : UDINT and writable : UDINT, and no return type
        |
        | Note: a stream's or a future's new gives its reading end, then its writing end, each a UDINT
    ----'
    [E1516] Error: stream built-in that does not fit its declaration
        ,-[ file:///test0.st:32:10 ]
        |
     32 | FUNCTION NEW_FUTURE
        |          ^^^^^|^^^^
        |               `------ 'NEW_FUTURE' does not declare the two ends it makes
        |
        | Help: declare the VAR_OUTPUTs readable : UDINT and writable : UDINT, and no return type
        |
        | Note: a stream's or a future's new gives its reading end, then its writing end, each a UDINT
    ----'
    ");
}

/// Async imports from WASI and from a host's module: parameters, a STRING
/// among them past the 4 values a call passes, and the fields of a result.
/// Accepted, and the module they build is valid.
#[rstest]
fn valid_async_imports(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
FUNCTION_BLOCK WAIT_FOR
VAR_INPUT how_long : LTIME; END_VAR
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{extern async 'probe' 'fetch'}
FUNCTION_BLOCK FETCH
VAR_INPUT a : DINT; b : DINT; c : DINT; label : STRING[8]; END_VAR
VAR_OUTPUT DONE : BOOL; value : UDINT; ok : BOOL; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR wait : WAIT_FOR; fetch : FETCH; value : UDINT; END_VAR
    wait(how_long := LT#1s);
    fetch(a := 1, b := 2, c := 3, label := 'x');
    IF fetch.DONE THEN
        value := fetch.value;
    END_IF;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// An import from a module other than WASI's is typed in WIT from its
/// declaration: every scalar has a WIT type, an enum and a subrange the one
/// they are stored as, a STRING input is a `string`, and the return value is
/// the one result.
#[rstest]
fn valid_extern_from_a_host_module(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mode : (Off, Slow, Fast); END_TYPE
TYPE Percent : INT (0..100); END_TYPE

{extern 'modbus' 'write-numbers'}
FUNCTION WriteNumbers : BOOL
VAR_INPUT
    flag : BOOL;
    s8 : SINT; s16 : INT; s32 : DINT; s64 : LINT;
    u8 : USINT; u16 : UINT; u32 : UDINT; u64 : ULINT;
    f32 : REAL; f64 : LREAL;
END_VAR
END_FUNCTION

{extern 'modbus' 'write-the-rest'}
FUNCTION WriteTheRest : CHAR
VAR_INPUT
    b8 : BYTE; b16 : WORD; b32 : DWORD; b64 : LWORD;
    letter : CHAR;
    text : STRING[20];
    mode : Mode;
    level : Percent;
END_VAR
END_FUNCTION

{extern 'modbus' 'read-clock'}
FUNCTION ReadClock : LDT
VAR_INPUT t : TIME; lt : LTIME; d : DATE; ld : LDATE; time_of_day : TOD; long_time_of_day : LTOD; stamp : DT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A module is a WIT name or an interface, with or without its version,
/// and a name is a WIT name: words of lowercase letters or of capitals,
/// with digits, a word after the first may start with a digit.
#[rstest]
fn valid_extern_wit_names(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus-rtu' 'read-coils'}
FUNCTION A : INT
END_FUNCTION

{extern 'acme:modbus/rtu' 'HTTP-get'}
FUNCTION B : INT
END_FUNCTION

{extern 'acme:modbus/RTU@1.2.3-beta.1+build.5' 'axis-2'}
FUNCTION C : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A module is written as WIT writes it, never converted: the host defines
/// the name the declaration gives. What would be a WIT name is suggested.
#[rstest]
fn invalid_extern_module_not_a_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'my_host' 'now'}
FUNCTION A : INT
END_FUNCTION

{extern 'Modbus' 'now'}
FUNCTION B : INT
END_FUNCTION

{extern 'acme:io@1.0.0' 'now'}
FUNCTION C : INT
END_FUNCTION

{extern 'Acme:io/poll' 'now'}
FUNCTION D : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'my_host' 'now'}
       |         ^^^^|^^^^
       |             `------ 'my_host' is not a WIT name
       |
       | Help: write 'my-host'
       |
       | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:9 ]
       |
     6 | {extern 'Modbus' 'now'}
       |         ^^^^|^^^
       |             `----- 'Modbus' is not a WIT name
       |
       | Help: write 'modbus'
       |
       | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ---'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:10:9 ]
        |
     10 | {extern 'acme:io@1.0.0' 'now'}
        |         ^^^^^^^|^^^^^^^
        |                `--------- 'acme:io@1.0.0' is not an interface's name
        |
        | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ----'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:14:9 ]
        |
     14 | {extern 'Acme:io/poll' 'now'}
        |         ^^^^^^^|^^^^^^
        |                `-------- 'Acme:io/poll' is not an interface's name
        |
        | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ----'
    ");
}

/// A second namespace or a second interface is a nested name, which the
/// component model's tools do not take yet.
#[rstest]
fn invalid_extern_module_nested(mut with_db: RootDatabase) {
    let source = r#"
{extern 'acme:modbus:rtu/read' 'now'}
FUNCTION A : INT
END_FUNCTION

{extern 'acme:modbus/rtu/read' 'now'}
FUNCTION B : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'acme:modbus:rtu/read' 'now'}
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'acme:modbus:rtu/read' is a nested name
       |
       | Note: an interface is `namespace:package/interface`, and the component model's tools take no nested name yet
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:9 ]
       |
     6 | {extern 'acme:modbus/rtu/read' 'now'}
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'acme:modbus/rtu/read' is a nested name
       |
       | Note: an interface is `namespace:package/interface`, and the component model's tools take no nested name yet
    ---'
    ");
}

/// An interface's version is a semantic version, as the component model
/// reads one.
#[rstest]
fn invalid_extern_module_version(mut with_db: RootDatabase) {
    let source = r#"
{extern 'acme:modbus/rtu@1' 'now'}
FUNCTION A : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'acme:modbus/rtu@1' 'now'}
       |         ^^^^^^^^^|^^^^^^^^^
       |                  `----------- '1' is not a semantic version
       |
       | Note: an interface's version is `MAJOR.MINOR.PATCH`, like `1.0.0`
    ---'
    ");
}

/// A function's name is a WIT name, WASI's too.
#[rstest]
fn invalid_extern_function_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'read_coils'}
FUNCTION A : INT
END_FUNCTION

{extern 'modbus' 'ReadCoils'}
FUNCTION B : INT
END_FUNCTION

{extern 'wasi:clocks/monotonic-clock@0.3.0' 'sqrt.REAL'}
FUNCTION C : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:18 ]
       |
     2 | {extern 'modbus' 'read_coils'}
       |                  ^^^^^^|^^^^^
       |                        `------- 'read_coils' is not a WIT name
       |
       | Help: write 'read-coils'
       |
       | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:18 ]
       |
     6 | {extern 'modbus' 'ReadCoils'}
       |                  ^^^^^|^^^^^
       |                       `------- 'ReadCoils' is not a WIT name
       |
       | Help: write 'read-coils'
       |
       | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:10:45 ]
        |
     10 | {extern 'wasi:clocks/monotonic-clock@0.3.0' 'sqrt.REAL'}
        |                                             ^^^^^|^^^^^
        |                                                  `------- 'sqrt.REAL' is not a WIT name
        |
        | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ----'
    ");
}

/// A parameter's WIT name is its name in lowercase with each `_` written
/// `-`: a name that leaves a word empty, or has a letter outside ASCII, has
/// none.
#[rstest]
fn invalid_extern_parameter_without_a_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'write'}
FUNCTION A : INT
VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |           ^|
       |            `-- 'x_' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:21 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                     ^|
       |                      `-- '_y' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:31 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                               ^^|^
       |                                 `--- 'a__b' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:43 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                                           ^^^^^|^^^^^
       |                                                `------- 'température' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    ");
}

/// The component model tells two names apart without their `-`, so `ab`
/// and `a_b` are one parameter to it.
#[rstest]
fn invalid_extern_parameters_with_one_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'write'}
FUNCTION A : INT
VAR_INPUT ab : INT; a_b : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:21 ]
       |
     4 | VAR_INPUT ab : INT; a_b : INT; END_VAR
       |           ^|        ^|^
       |            `------------- 'ab' is declared here
       |                      |
       |                      `--- 'a_b' cannot be told from 'ab' in WIT
       |
       | Note: the component model compares two names without their '-'
    ---'
    ");
}

/// An import from a module other than WASI's returns the record of its
/// VAR_OUTPUTs and its return value, typed in WIT: one output and no return
/// value is a record of one field.
#[rstest]
fn valid_extern_outputs_from_a_host_module(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mode : (Off, On); END_TYPE

{extern 'rt' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : REAL; mode : Mode; level : USINT (0..9); END_VAR
END_FUNCTION

{extern 'acme:rt/clock@1.0.0' 'tick'}
FUNCTION tick
VAR_OUTPUT count : ULINT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// Each VAR_OUTPUT of such an import is a field of its result, named in
/// WIT: a name that gives no WIT name, two that WIT takes for one, and a
/// name the return value has already, the import's own, are refused.
#[rstest]
fn invalid_extern_output_names(mut with_db: RootDatabase) {
    let source = r#"
{extern 'rt' 'sample'}
FUNCTION ReadSample : INT
VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Sample : REAL; END_VAR
END_FUNCTION

{extern 'rt' 'other'}
FUNCTION Other
VAR_OUTPUT other : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Sample : REAL; END_VAR
       |            ^^|^
       |              `--- 'x__y' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:35 ]
       |
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Sample : REAL; END_VAR
       |                        ^|^        ^|
       |                         `------------- 'a_b' is declared here
       |                                    |
       |                                    `-- 'ab' cannot be told from 'a_b' in WIT
       |
       | Note: the component model compares two names without their '-'
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:45 ]
       |
     3 | FUNCTION ReadSample : INT
       |          ^^^^^|^^^^
       |               `------ 'ReadSample' is declared here
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Sample : REAL; END_VAR
       |                                             ^^^|^^
       |                                                `---- 'Sample' cannot be told from the return value 'sample' in WIT
       |
       | Help: rename the VAR_OUTPUT
       |
       | Note: an {extern} FUNCTION returns its VAR_OUTPUTs and its return value as one record, the return value named as the import is
    ---'
    ");
}

/// The component model passes no address: a REF_TO, a STRUCT, an ARRAY and
/// an instance are refused as the inputs of an import from a module other
/// than WASI's.
#[rstest]
fn invalid_extern_reference_and_aggregate_inputs(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

{extern 'host' 'send'}
FUNCTION Send : INT
VAR_INPUT
    r : REF_TO INT;
    p : Pt;
    a : ARRAY[0..3] OF BYTE;
    c : Counter;
END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     r : REF_TO INT;
        |     |
        |     `-- VAR_INPUT 'r' cannot cross a component import: an address in the module means nothing to the host
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     p : Pt;
        |     |
        |     `-- VAR_INPUT 'p' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     a : ARRAY[0..3] OF BYTE;
        |     |
        |     `-- VAR_INPUT 'a' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     c : Counter;
        |     |
        |     `-- VAR_INPUT 'c' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    ");
}

/// An import takes 16 values, a STRING counting two: past them the
/// component model passes the values in memory, which a call does not.
#[rstest]
fn invalid_extern_more_than_16_values(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'sixteen'}
FUNCTION Sixteen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
END_VAR
END_FUNCTION

{extern 'host' 'seventeen'}
FUNCTION Seventeen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
    n : INT;
END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     n : INT;
        |     |
        |     `-- VAR_INPUT 'n' cannot cross a component import: the import would take more than 16 values
        |
        | Note: a component import takes 16 values at most, a STRING counting two
    ----'
    ");
}

#[rstest]
fn valid_test_pragma_referencing_normal_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A test cannot call another test either. The runner is the only caller: a
/// `{test}` FUNCTION is emitted with the runner's signature, not the one it
/// declares, so a call produced a module that failed to load, or handed back
/// the runner's pass/fail code in place of the declared value.
#[rstest]
fn invalid_test_pragma_referencing_test_pou(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := test_helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Error: access to a test-only item
       ,-[ file:///test0.st:9:10 ]
       |
     9 |     x := test_helper();
       |          ^^^^^|^^^^^
       |               `------- cannot access test item 'test_helper'
       |
       | Help: for code shared between tests, write a POU without the pragma
       |
       | Note: a {test} POU is the test runner's entry point, not a callable
    ---'
    ");
}

#[rstest]
fn invalid_non_test_referencing_test_function(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_only : INT
END_FUNCTION

FUNCTION normal : INT
VAR x : INT; END_VAR
    x := test_only();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Error: access to a test-only item
       ,-[ file:///test0.st:8:10 ]
       |
     8 |     x := test_only();
       |          ^^^^|^^^^
       |              `------ cannot access test item 'test_only'
       |
       | Help: for code shared between tests, write a POU without the pragma
       |
       | Note: a {test} POU is the test runner's entry point, not a callable
    ---'
    ");
}

// E1505/E1504: a {wasm} pragma is validated where it is written. An unknown
// name used to fall through to `unreachable` (or, on the conversion shape,
// to a silent identity), and a pragma outside a FUNCTION was silently
// dropped — all at exit 0.

#[rstest]
fn invalid_unknown_wasm_instruction(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION BOGUS : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            {wasm 'not.a.real.instruction' (params a b) (result BOGUS)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'not.a.real.instruction' (params a b) (result BOGUS)}
       |                   ^^^^^^^^^^^^|^^^^^^^^^^^
       |                               `------------- 'not.a.real.instruction' is not a wasm instruction this compiler emits
    ---'
    ");
}

#[rstest]
fn invalid_unknown_wasm_instruction_with_type_basis(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION BOGUS : INT
        VAR_INPUT a : INT; END_VAR
            {wasm a 'zorble' (params a) (result BOGUS)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:21 ]
       |
     4 |             {wasm a 'zorble' (params a) (result BOGUS)}
       |                     ^^^^|^^^
       |                         `----- 'zorble' is not a wasm instruction this compiler emits
    ---'
    ");
}

// E1506: a pragma's operands are the names it gives, resolved against the
// FUNCTION. An unknown one used to be ignored while the FUNCTION's own
// parameter list was lowered instead.

#[rstest]
fn invalid_wasm_operand_not_declared(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Root : REAL
        VAR_INPUT a : REAL; END_VAR
            {wasm 'f32.sqrt' (params b) (result Root)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1506] Error: wasm operand not declared
       ,-[ file:///test0.st:4:38 ]
       |
     4 |             {wasm 'f32.sqrt' (params b) (result Root)}
       |                                      |
       |                                      `-- 'b' is not a parameter, a local or the return of this FUNCTION
    ---'
    ");
}

#[rstest]
fn invalid_wasm_result_on_a_function_without_a_return(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Root
        VAR_INPUT a : REAL; END_VAR
            {wasm 'f32.sqrt' (params a) (result Root)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1506] Error: wasm operand not declared
       ,-[ file:///test0.st:4:49 ]
       |
     4 |             {wasm 'f32.sqrt' (params a) (result Root)}
       |                                                 ^^|^
       |                                                   `--- 'Root' is not a parameter, a local or the return of this FUNCTION
    ---'
    ");
}

#[rstest]
fn valid_wasm_pragmas_write_locals_in_any_case(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION round_sat : DINT
        VAR_INPUT IN : REAL; END_VAR
        VAR r : REAL; END_VAR
            {wasm 'f32.nearest' (params in) (result R)}
            {wasm 'i32.trunc_sat_f32_s' (params r) (result ROUND_SAT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// E1507: a pragma's operands must fit its instruction, lane for lane. The
// module validator used to be the first to say so, at load, from a compile
// that exited 0.

#[rstest]
fn invalid_wasm_operand_in_the_wrong_lane(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION NEAREST : DINT
        VAR_INPUT IN : DINT; END_VAR
            {wasm 'f32.nearest' (params IN) (result NEAREST)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.nearest' (params IN) (result NEAREST)}
       |                   ^^^^^^|^^^^^^
       |                         `-------- 'f32.nearest' takes (f32) -> f32, not (IN: DINT (i32)) -> NEAREST: DINT (i32)
    ---'
    ");
}

#[rstest]
fn invalid_wasm_operand_count(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION ROOT : REAL
        VAR_INPUT a : REAL; b : REAL; END_VAR
            {wasm 'f32.sqrt' (params a b) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.sqrt' (params a b) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32, not (a: REAL (f32), b: REAL (f32)) -> ROOT: REAL (f32)
    ---'
    ");
}

/// A conversion's written instruction must be the one its lanes name, even
/// though the cast road picks the op by the types: the text is what a
/// reader trusts.
#[rstest]
fn invalid_wasm_result_in_the_wrong_lane(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION TRUNCATE : REAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNCATE)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNCATE)}
       |                   ^^^^^^^^^^|^^^^^^^^^^
       |                             `------------ 'i32.trunc_sat_f32_s' takes (f32) -> i32, not (IN: REAL (f32)) -> TRUNCATE: REAL (f32)
    ---'
    ");
}

#[rstest]
fn invalid_wasm_string_operand_on_a_numeric_instruction(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION LENGTH : DINT
        VAR_INPUT s : STRING; END_VAR
            {wasm 'str.byte_len' (params s) (result LENGTH)}
        END_FUNCTION

        FUNCTION ROOT : REAL
        VAR_INPUT s : STRING; END_VAR
            {wasm 'f32.sqrt' (params s) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:9:19 ]
       |
     9 |             {wasm 'f32.sqrt' (params s) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32, not (s: STRING (i32, i32)) -> ROOT: REAL (f32)
    ---'
    ");
}

/// A type basis resolves to a lane the instruction may not exist for.
#[rstest]
fn invalid_wasm_type_basis_without_a_form(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SHIFT : BOOL
        VAR_INPUT IN : BOOL; N : INT; END_VAR
            {wasm IN 'shl' (params IN N) (result SHIFT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             {wasm IN 'shl' (params IN N) (result SHIFT)}
       |                      ^^|^^
       |                        `---- 'rk.shl1' is not a wasm instruction this compiler emits
    ---'
    ");
}

/// One byte has no order, and a REAL has no byte swap: `rk.bswap8` and
/// `f32.bswap` are no instructions.
#[rstest]
fn invalid_wasm_byte_swap_without_a_form(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SWAP8 : BYTE
        VAR_INPUT IN : BYTE; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAP8)}
        END_FUNCTION

        FUNCTION SWAPR : REAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAPR)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             {wasm IN 'bswap' (params IN) (result SWAP8)}
       |                      ^^^|^^^
       |                         `----- 'rk.bswap8' is not a wasm instruction this compiler emits
    ---'
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:9:22 ]
       |
     9 |             {wasm IN 'bswap' (params IN) (result SWAPR)}
       |                      ^^^|^^^
       |                         `----- 'f32.bswap' is not a wasm instruction this compiler emits
    ---'
    ");
}

/// The shapes the library relies on all fit: a STRING producer, a STRING
/// consumer, a 64-bit rotate with an INT count, a widening with a basis.
#[rstest]
fn valid_wasm_signatures(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION JOIN : STRING
        VAR_INPUT a : STRING; b : STRING; END_VAR
            {wasm 'str.concat' (params a b) (result JOIN)}
        END_FUNCTION

        FUNCTION LENGTH : UDINT
        VAR_INPUT s : STRING; END_VAR
            {wasm 'str.byte_len' (params s) (result LENGTH)}
        END_FUNCTION

        FUNCTION ROTATE : LWORD
        VAR_INPUT IN : LWORD; N : INT; END_VAR
            {wasm IN 'rotl' (params IN N) (result ROTATE)}
        END_FUNCTION

        FUNCTION WIDEN : LREAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm 'f64.promote_f32' (params IN) (result WIDEN)}
        END_FUNCTION

        FUNCTION ROOT : LREAL
        VAR_INPUT IN : LREAL; END_VAR
            {wasm IN 'sqrt' (params IN) (result ROOT)}
        END_FUNCTION

        FUNCTION SWAP : LWORD
        VAR_INPUT IN : LWORD; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAP)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_wasm_pragma_outside_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            {wasm 'i32.shl' (params a a) (result o)}
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1504] Error: wasm pragma outside a FUNCTION
       ,-[ file:///test0.st:5:19 ]
       |
     5 |             {wasm 'i32.shl' (params a a) (result o)}
       |                   ^^^^|^^^^
       |                       `------ a {wasm} body is only available on a FUNCTION
    ---'
    ");
}

// The four families stay clean: a native name, a builtin, a bare op that
// prefixes on its type basis, and the conversion pseudo-names.
#[rstest]
fn valid_wasm_instruction_names(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SHIFT : DINT
        VAR_INPUT a : DINT; n : DINT; END_VAR
            {wasm 'i32.shl' (params a n) (result SHIFT)}
        END_FUNCTION

        FUNCTION SINE : LREAL
        VAR_INPUT x : LREAL; END_VAR
            {wasm 'f64.sin' (params x) (result SINE)}
        END_FUNCTION

        FUNCTION SINB : LREAL
        VAR_INPUT x : LREAL; END_VAR
            {wasm x 'sin' (params x) (result SINB)}
        END_FUNCTION

        FUNCTION WIDEN : LINT
        VAR_INPUT x : DINT; END_VAR
            {wasm 'cast' (params x) (result WIDEN)}
        END_FUNCTION

        FUNCTION SAME : DINT
        VAR_INPUT x : DINT; END_VAR
            {wasm 'nop' (params x) (result SAME)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The return type is the import's LAST result, so the scalar rule that
/// governs VAR_OUTPUT governs it too. It is also the one place an aggregate
/// could still be written: checking only the sections let a STRING return
/// through to an assertion in codegen.
#[rstest]
fn invalid_extern_aggregate_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'greet'}
FUNCTION Greet : STRING
VAR_INPUT who : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:3:18 ]
       |
     3 | FUNCTION Greet : STRING
       |                  ^^^|^^
       |                     `---- the return type of 'Greet' can only be a scalar
    ---'
    ");
}

/// A scalar return is the ordinary case and stays legal.
#[rstest]
fn valid_extern_scalar_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'now'}
FUNCTION Now : LWORD
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// `{test}` is on a FUNCTION, which the runner calls once, or on a
/// FUNCTION_BLOCK, which it calls cycle by cycle on an instance of its own: a
/// PROGRAM and a METHOD have no such entry, and an ABSTRACT or `{extern}`
/// block no instance the runner can make. A helper that only tests use is
/// hidden with `FUNCTION PRIVATE`, not marked as a test.
#[rstest]
fn invalid_test_pragma_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK ABSTRACT test_abstract
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{test}
{extern async 'host' 'poll'}
FUNCTION_BLOCK test_import
VAR_OUTPUT DONE : BOOL; END_VAR
END_FUNCTION_BLOCK

{test}
PROGRAM prog
END_PROGRAM

CLASS C
    {test}
    METHOD m : INT
        m := 1;
    END_METHOD
END_CLASS

{test}
FUNCTION t : INT
    t := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1503] Error: test pragma outside a FUNCTION or FUNCTION_BLOCK
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a FUNCTION_BLOCK declared ABSTRACT
       |
       | Note: a test is a FUNCTION, which the runner calls once, or a FUNCTION_BLOCK, which it calls cycle by cycle on an instance of its own
    ---'
    [E1503] Error: test pragma outside a FUNCTION or FUNCTION_BLOCK
       ,-[ file:///test0.st:7:1 ]
       |
     7 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a FUNCTION_BLOCK declared {extern}
       |
       | Note: a test is a FUNCTION, which the runner calls once, or a FUNCTION_BLOCK, which it calls cycle by cycle on an instance of its own
    ---'
    [E1503] Error: test pragma outside a FUNCTION or FUNCTION_BLOCK
        ,-[ file:///test0.st:13:1 ]
        |
     13 | {test}
        | ^^^|^^
        |    `---- a {test} pragma cannot be placed on a PROGRAM
        |
        | Note: a test is a FUNCTION, which the runner calls once, or a FUNCTION_BLOCK, which it calls cycle by cycle on an instance of its own
    ----'
    [E1503] Error: test pragma outside a FUNCTION or FUNCTION_BLOCK
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     {test}
        |     ^^^|^^
        |        `---- a {test} pragma cannot be placed on a METHOD
        |
        | Note: a test is a FUNCTION, which the runner calls once, or a FUNCTION_BLOCK, which it calls cycle by cycle on an instance of its own
    ----'
    ");
}

/// A `{test}` FUNCTION_BLOCK runs cycle by cycle on an instance the runner
/// makes, until a cycle leaves its DONE TRUE. It may declare other outputs,
/// methods and a base, and hold an async import, which a test FUNCTION
/// cannot.
#[rstest]
fn valid_test_block(mut with_db: RootDatabase) {
    let source = r#"
{extern async 'host' 'fetch'}
FUNCTION_BLOCK FETCH
VAR_OUTPUT DONE : BOOL; value : DINT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Fixture
VAR count : INT; END_VAR
    count := count + 1;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_fetches EXTENDS Fixture
VAR_OUTPUT DONE : BOOL; seen : DINT; END_VAR
VAR fetch : FETCH; END_VAR

METHOD checked : BOOL
    checked := count > 0;
END_METHOD

    SUPER();
    fetch();
    IF fetch.DONE THEN
        seen := fetch.value;
        DONE := THIS.checked();
    END_IF;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A test block declares its DONE, a BOOL in VAR_OUTPUT: the runner reads it
/// after each cycle.
#[rstest]
fn invalid_test_block_without_its_done(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK test_none
VAR_OUTPUT finished : BOOL; END_VAR
    finished := TRUE;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_in_var
VAR done : BOOL; END_VAR
    done := TRUE;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_not_bool
VAR_OUTPUT DONE : INT; END_VAR
    DONE := 1;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1517] Error: test FUNCTION_BLOCK without its DONE
       ,-[ file:///test0.st:3:16 ]
       |
     3 | FUNCTION_BLOCK test_none
       |                ^^^^|^^^^
       |                    `------ 'test_none' has no VAR_OUTPUT DONE
       |
       | Help: declare DONE : BOOL in VAR_OUTPUT
       |
       | Note: the runner calls a {test} FUNCTION_BLOCK once per cycle, until a cycle leaves its DONE TRUE
    ---'
    [E1517] Error: test FUNCTION_BLOCK without its DONE
        ,-[ file:///test0.st:10:5 ]
        |
     10 | VAR done : BOOL; END_VAR
        |     ^^|^
        |       `--- 'done' is not a VAR_OUTPUT of type BOOL
        |
        | Help: declare DONE : BOOL in VAR_OUTPUT
        |
        | Note: the runner calls a {test} FUNCTION_BLOCK once per cycle, until a cycle leaves its DONE TRUE
    ----'
    [E1517] Error: test FUNCTION_BLOCK without its DONE
        ,-[ file:///test0.st:16:12 ]
        |
     16 | VAR_OUTPUT DONE : INT; END_VAR
        |            ^^|^
        |              `--- 'DONE' is not a VAR_OUTPUT of type BOOL
        |
        | Help: declare DONE : BOOL in VAR_OUTPUT
        |
        | Note: the runner calls a {test} FUNCTION_BLOCK once per cycle, until a cycle leaves its DONE TRUE
    ----'
    ");
}

/// A test block takes no input: the runner passes none. Its VAR_OUTPUTs are
/// its instance's, DONE among them.
#[rstest]
fn invalid_test_block_with_a_parameter(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK test_inputs
VAR_INPUT limit : INT; END_VAR
VAR_IN_OUT shared : INT; END_VAR
VAR_OUTPUT DONE : BOOL; count : INT; END_VAR
    count := count + shared;
    DONE := count >= limit;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1511] Error: test with a parameter
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT limit : INT; END_VAR
       |           ^^|^^
       |             `---- the {test} FUNCTION_BLOCK cannot take the parameter 'limit'
       |
       | Help: declare it in VAR, or call a FUNCTION taking it from the test
       |
       | Note: the runner calls a test with no arguments
    ---'
    [E1511] Error: test with a parameter
       ,-[ file:///test0.st:5:12 ]
       |
     5 | VAR_IN_OUT shared : INT; END_VAR
       |            ^^^|^^
       |               `---- the {test} FUNCTION_BLOCK cannot take the parameter 'shared'
       |
       | Help: declare it in VAR, or call a FUNCTION taking it from the test
       |
       | Note: the runner calls a test with no arguments
    ---'
    ");
}

/// Nothing names a test block, as nothing names a test FUNCTION: the runner
/// makes its instance, and a library's test blocks are left out of its
/// users' builds.
#[rstest]
fn invalid_reference_to_a_test_block(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK test_block
VAR_OUTPUT DONE : BOOL; END_VAR
    DONE := TRUE;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS test_block
END_FUNCTION_BLOCK

PROGRAM P
VAR t : test_block; END_VAR
    t();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Error: access to a test-only item
       ,-[ file:///test0.st:8:32 ]
       |
     8 | FUNCTION_BLOCK Derived EXTENDS test_block
       |                                ^^^^^|^^^^
       |                                     `------ cannot access test item 'test_block'
       |
       | Help: for code shared between tests, write a POU without the pragma
       |
       | Note: a {test} POU is the test runner's entry point, not a callable
    ---'
    [E1007] Error: access to a test-only item
        ,-[ file:///test0.st:12:9 ]
        |
     12 | VAR t : test_block; END_VAR
        |         ^^^^^|^^^^
        |              `------ cannot access test item 'test_block'
        |
        | Help: for code shared between tests, write a POU without the pragma
        |
        | Note: a {test} POU is the test runner's entry point, not a callable
    ----'
    ");
}

/// An export is typed in WIT from its declaration: every scalar has a WIT
/// type, an enum and a subrange the one they are stored as, a STRING goes
/// both ways, and a name the pragma gives stands for the FUNCTION's own.
#[rstest]
fn valid_export_typed_in_wit(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mode : (Off, Slow, Fast); END_TYPE
TYPE Percent : INT (0..100); END_TYPE

{export}
FUNCTION Take_Numbers : LREAL
VAR_INPUT
    flag : BOOL;
    s8 : SINT; s16 : INT; s32 : DINT; s64 : LINT;
    u8 : USINT; u16 : UINT; u32 : UDINT; u64 : ULINT;
    f32 : REAL; f64 : LREAL;
END_VAR
    Take_Numbers := f64;
END_FUNCTION

{export 'take-the-rest'}
FUNCTION TakeTheRest : STRING
VAR_INPUT
    b8 : BYTE; b16 : WORD; b32 : DWORD; b64 : LWORD;
    letter : CHAR;
    text : STRING[20];
    mode : Mode;
    level : Percent;
    since : TIME;
    stamp : LDT;
END_VAR
    TakeTheRest := text;
END_FUNCTION

NAMESPACE Motion.Axis
    {export}
    FUNCTION Level : Percent
        Level := 50;
    END_FUNCTION

    {export}
    FUNCTION Current : Mode
        Current := Mode#Slow;
    END_FUNCTION

    {export}
    FUNCTION Stop
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A host passes values: an export has no VAR_IN_OUT. Its VAR_OUTPUTs are
/// fields of the record it returns.
#[rstest]
fn invalid_export_in_out(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Scale : REAL
VAR_INPUT raw : INT; END_VAR
VAR_IN_OUT total : REAL; END_VAR
VAR_OUTPUT clamped : BOOL; END_VAR
    Scale := raw;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:5:12 ]
       |
     5 | VAR_IN_OUT total : REAL; END_VAR
       |            ^^|^^
       |              `---- VAR_IN_OUT 'total' cannot cross a component export: the host passes values, not references
       |
       | Note: an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT
    ---'
    ");
}

/// An export returns a record of scalars and STRINGs: a STRUCT, an ARRAY,
/// an instance or a reference output is refused, and a STRING is not.
#[rstest]
fn invalid_export_aggregate_output(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

{export}
FUNCTION Read : INT
VAR_OUTPUT
    p : Pt;
    a : ARRAY[0..3] OF BYTE;
    c : Counter;
    r : REF_TO INT;
    text : STRING[12];
END_VAR
    Read := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     p : Pt;
        |     |
        |     `-- VAR_OUTPUT 'p' cannot be a field of the export's result: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars and STRINGs
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     a : ARRAY[0..3] OF BYTE;
        |     |
        |     `-- VAR_OUTPUT 'a' cannot be a field of the export's result: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars and STRINGs
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     c : Counter;
        |     |
        |     `-- VAR_OUTPUT 'c' cannot be a field of the export's result: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars and STRINGs
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     r : REF_TO INT;
        |     |
        |     `-- VAR_OUTPUT 'r' cannot be a field of the export's result: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars and STRINGs
    ----'
    ");
}

/// The VAR_OUTPUTs of an export are named in WIT as an import's are: the
/// return value is the field named as the export is, `level` below.
#[rstest]
fn invalid_export_output_names(mut with_db: RootDatabase) {
    let source = r#"
{export 'level'}
FUNCTION Read : INT
VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Level : REAL; END_VAR
    Read := 0;
END_FUNCTION

NAMESPACE Tank
    {export}
    FUNCTION Fill : INT
    VAR_OUTPUT tank_fill : BOOL; END_VAR
        Fill := 0;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Level : REAL; END_VAR
       |            ^^|^
       |              `--- 'x__y' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:4:35 ]
       |
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Level : REAL; END_VAR
       |                        ^|^        ^|
       |                         `------------- 'a_b' is declared here
       |                                    |
       |                                    `-- 'ab' cannot be told from 'a_b' in WIT
       |
       | Note: the component model compares two names without their '-'
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:4:45 ]
       |
     3 | FUNCTION Read : INT
       |          ^^|^
       |            `--- 'Read' is declared here
     4 | VAR_OUTPUT x__y : INT; a_b : INT; ab : INT; Level : REAL; END_VAR
       |                                             ^^|^^
       |                                               `---- 'Level' cannot be told from the return value 'level' in WIT
       |
       | Help: rename the VAR_OUTPUT
       |
       | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record, the return value named as the export is
    ---'
    [E1513] Error: export name not a WIT name
        ,-[ file:///test0.st:11:16 ]
        |
     10 |     FUNCTION Fill : INT
        |              ^^|^
        |                `--- 'Fill' is declared here
     11 |     VAR_OUTPUT tank_fill : BOOL; END_VAR
        |                ^^^^|^^^^
        |                    `------ 'tank_fill' cannot be told from the return value 'tank-fill' in WIT
        |
        | Help: rename the VAR_OUTPUT
        |
        | Note: an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record, the return value named as the export is
    ----'
    ");
}

/// The host passes no address and gets none back: a REF_TO, a STRUCT, an
/// ARRAY and an instance are refused as an export's inputs and as what it
/// returns.
#[rstest]
fn invalid_export_reference_and_aggregate(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

{export}
FUNCTION Send : INT
VAR_INPUT
    r : REF_TO INT;
    p : Pt;
    a : ARRAY[0..3] OF BYTE;
    c : Counter;
END_VAR
    Send := 0;
END_FUNCTION

{export}
FUNCTION Origin : Pt
END_FUNCTION

{export}
FUNCTION Nowhere : PInt
    Nowhere := NULL;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     r : REF_TO INT;
        |     |
        |     `-- VAR_INPUT 'r' cannot cross a component export: the host has no address in the module to pass
        |
        | Note: an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     p : Pt;
        |     |
        |     `-- VAR_INPUT 'p' cannot cross a component export: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     a : ARRAY[0..3] OF BYTE;
        |     |
        |     `-- VAR_INPUT 'a' cannot cross a component export: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     c : Counter;
        |     |
        |     `-- VAR_INPUT 'c' cannot cross a component export: only a scalar or a STRING can
        |
        | Note: an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:21:19 ]
        |
     21 | FUNCTION Origin : Pt
        |                   ^|
        |                    `-- the return type of 'Origin' can only be a scalar or a STRING
        |
        | Note: an {export} FUNCTION returns one WIT value
    ----'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:25:20 ]
        |
     25 | FUNCTION Nowhere : PInt
        |                    ^^|^
        |                      `--- the return type of 'Nowhere' can only be a scalar or a STRING
        |
        | Note: an {export} FUNCTION returns one WIT value
    ----'
    ");
}

/// An export takes 16 values, a STRING counting two.
#[rstest]
fn invalid_export_more_than_16_values(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Sixteen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
END_VAR
    Sixteen := 0;
END_FUNCTION

{export}
FUNCTION Seventeen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
    n : INT;
END_VAR
    Seventeen := n;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:16:5 ]
        |
     16 |     n : INT;
        |     |
        |     `-- VAR_INPUT 'n' cannot cross a component export: the export would take more than 16 values
        |
        | Note: a component export takes 16 values at most, a STRING counting two
    ----'
    ");
}

/// The name a pragma gives is written as WIT writes it.
#[rstest]
fn invalid_export_given_name(mut with_db: RootDatabase) {
    let source = r#"
{export 'Read_Coils'}
FUNCTION A : INT
    A := 0;
END_FUNCTION

{export ''}
FUNCTION B : INT
    B := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {export 'Read_Coils'}
       |         ^^^^^^|^^^^^
       |               `------- 'Read_Coils' is not a WIT name
       |
       | Help: write 'read-coils'
       |
       | Note: an export's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:7:9 ]
       |
     7 | {export ''}
       |         ^|
       |          `-- '' is not a WIT name
       |
       | Note: an export's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    ");
}

/// An export is named by its namespaces' names and its own: one that
/// leaves a word empty, or has a letter outside ASCII, gives no WIT name,
/// and the pragma can give one.
#[rstest]
fn invalid_export_without_a_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION _Reset : INT
    _Reset := 0;
END_FUNCTION

NAMESPACE Été
    {export}
    FUNCTION Start : INT
        Start := 0;
    END_FUNCTION

    {export 'summer-stop'}
    FUNCTION Stop : INT
        Stop := 0;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:3:10 ]
       |
     3 | FUNCTION _Reset : INT
       |          ^^^|^^
       |             `---- '_Reset' has no WIT name
       |
       | Help: name the export, {export 'name'}
       |
       | Note: an export's WIT name is its namespaces' names and its own in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:9:14 ]
       |
     9 |     FUNCTION Start : INT
       |              ^^|^^
       |                `---- 'été' has no WIT name
       |
       | Help: name the export, {export 'name'}
       |
       | Note: an export's WIT name is its namespaces' names and its own in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    ");
}

/// Two exports with one WIT name are one to a host: each is reported,
/// with the other. `Motion.Scale_Axis`, `Motion_Scale.Axis` and
/// `motionscale.axis` all come to `motion-scale-axis`, which the component
/// model compares without its `-`. A name given by the pragma tells them
/// apart.
#[rstest]
fn invalid_exports_with_one_wit_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Motion
    {export}
    FUNCTION Scale_Axis : INT
        Scale_Axis := 0;
    END_FUNCTION
END_NAMESPACE

NAMESPACE Motion_Scale
    {export}
    FUNCTION Axis : INT
        Axis := 0;
    END_FUNCTION
END_NAMESPACE

NAMESPACE MotionScale
    {export 'motion-scale-axis-2'}
    FUNCTION Axis : INT
        Axis := 0;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1513] Error: export name not a WIT name
        ,-[ file:///test0.st:4:14 ]
        |
      4 |     FUNCTION Scale_Axis : INT
        |              ^^^^^|^^^^
        |                   `------ 'Scale_Axis' is exported as 'motion-scale-axis', which cannot be told from the export of 'Axis'
        |
     11 |     FUNCTION Axis : INT
        |              ^^|^
        |                `--- 'Axis' is declared here
        |
        | Help: name one of them, {export 'name'}
        |
        | Note: an export's WIT name is its namespaces' names and its own, joined by '-', and the component model compares two names without their '-'
    ----'
    [E1513] Error: export name not a WIT name
        ,-[ file:///test0.st:11:14 ]
        |
      4 |     FUNCTION Scale_Axis : INT
        |              ^^^^^|^^^^
        |                   `------ 'Scale_Axis' is declared here
        |
     11 |     FUNCTION Axis : INT
        |              ^^|^
        |                `--- 'Axis' is exported as 'motion-scale-axis', which cannot be told from the export of 'Scale_Axis'
        |
        | Help: name one of them, {export 'name'}
        |
        | Note: an export's WIT name is its namespaces' names and its own, joined by '-', and the component model compares two names without their '-'
    ----'
    ");
}

/// The two exports may be in two files: each file reports its own.
#[rstest]
fn invalid_exports_with_one_wit_name_in_two_files(mut with_db: RootDatabase) {
    let first = r#"
{export}
FUNCTION Read_Coils : INT
    Read_Coils := 0;
END_FUNCTION
"#;
    let second = r#"
{export 'read-coils'}
FUNCTION Other : INT
    Other := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[first, second]), @r"
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:3:10 ]
       |
     3 | FUNCTION Read_Coils : INT
       |          ^^^^^|^^^^
       |               `------ 'Read_Coils' is exported as 'read-coils', which cannot be told from the export of 'Other'
       |
       |-[ file:///test1.st:3:10 ]
       |
     3 | FUNCTION Other : INT
       |          ^^|^^
       |            `---- 'Other' is declared here
       |
       | Help: name one of them, {export 'name'}
       |
       | Note: an export's WIT name is its namespaces' names and its own, joined by '-', and the component model compares two names without their '-'
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test1.st:3:10 ]
       |
     3 | FUNCTION Other : INT
       |          ^^|^^
       |            `---- 'Other' is exported as 'read-coils', which cannot be told from the export of 'Read_Coils'
       |
       |-[ file:///test0.st:3:10 ]
       |
     3 | FUNCTION Read_Coils : INT
       |          ^^^^^|^^^^
       |               `------ 'Read_Coils' is declared here
       |
       | Help: name one of them, {export 'name'}
       |
       | Note: an export's WIT name is its namespaces' names and its own, joined by '-', and the component model compares two names without their '-'
    ---'
    ");
}

/// An export's parameters are named in WIT as an import's are.
#[rstest]
fn invalid_export_parameter_names(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Write : INT
VAR_INPUT x_ : INT; ab : INT; a_b : INT; END_VAR
    Write := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT x_ : INT; ab : INT; a_b : INT; END_VAR
       |           ^|
       |            `-- 'x_' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1513] Error: export name not a WIT name
       ,-[ file:///test0.st:4:31 ]
       |
     4 | VAR_INPUT x_ : INT; ab : INT; a_b : INT; END_VAR
       |                     ^|        ^|^
       |                      `------------- 'ab' is declared here
       |                                |
       |                                `--- 'a_b' cannot be told from 'ab' in WIT
       |
       | Note: the component model compares two names without their '-'
    ---'
    ");
}

/// `{export}` makes a FUNCTION one a host calls, and nothing else does.
#[rstest]
fn valid_export_pragma(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Scale : REAL
VAR_INPUT raw : INT; gain : REAL; END_VAR
    Scale := raw * gain;
END_FUNCTION

{once}
{export}
FUNCTION Setup : INT
    Setup := 1;
END_FUNCTION

NAMESPACE Plant
    {export}
    FUNCTION Reset : INT
        Reset := 0;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A host can only call a FUNCTION. A PROGRAM is exported for the schedule
/// already, and an FB or a METHOD needs an instance the host does not have.
#[rstest]
fn invalid_export_pragma_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION_BLOCK fb
END_FUNCTION_BLOCK

{export}
PROGRAM prog
END_PROGRAM

FUNCTION_BLOCK holder
    {export}
    METHOD m : INT
        m := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1508] Error: export pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- an {export} pragma cannot be placed on a FUNCTION_BLOCK
       |
       | Note: only a FUNCTION can be {export}
    ---'
    [E1508] Error: export pragma outside a FUNCTION
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {export}
       | ^^^^|^^^
       |     `----- an {export} pragma cannot be placed on a PROGRAM
       |
       | Note: only a FUNCTION can be {export}
    ---'
    [E1508] Error: export pragma outside a FUNCTION
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     {export}
        |     ^^^^|^^^
        |         `----- an {export} pragma cannot be placed on a METHOD
        |
        | Note: only a FUNCTION can be {export}
    ----'
    ");
}

/// An import has no body to export, and a test is exported for the runner
/// under the same name, in a debug build only.
#[rstest]
fn invalid_export_on_an_extern_or_a_test(mut with_db: RootDatabase) {
    let source = r#"
{export}
{extern 'host' 'now'}
FUNCTION HostNow : LINT
END_FUNCTION

{test}
{export}
FUNCTION checks_something
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'HostNow' cannot be exported: it is an {extern} FUNCTION
       |
       | Note: an {extern} FUNCTION is an import, it has no body to export
    ---'
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:8:1 ]
       |
     8 | {export}
       | ^^^^|^^^
       |     `----- 'checks_something' cannot be exported: it is a {test} FUNCTION
       |
       | Note: a {test} FUNCTION is already exported for `rk test` and left out of a release build
    ---'
    ");
}

/// One declaration the lowering turns into several functions has no single
/// export to give: `drive$@Worker`, `sum_all$3`.
#[rstest]
fn invalid_export_on_a_function_lowered_more_than_once(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IWork
    METHOD Run : INT END_METHOD
END_INTERFACE

{export}
FUNCTION drive : INT
VAR_IN_OUT dev : IWork; END_VAR
    drive := dev.Run();
END_FUNCTION

{export}
FUNCTION sum_all : INT
VAR_INPUT args : INT...; END_VAR
    sum_all := ...args+;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {export}
       | ^^^^|^^^
       |     `----- 'drive' cannot be exported: it takes an interface
       |
       | Note: it is compiled once per implementation it is called with, so there is no single function to export
    ---'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:12:1 ]
        |
     12 | {export}
        | ^^^^|^^^
        |     `----- 'sum_all' cannot be exported: it is variadic
        |
        | Note: it is compiled once per number of arguments it is called with, so there is no single function to export
    ----'
    ");
}

/// An overloaded FUNCTION's symbol carries its signature (`Twice$INT`), and a
/// host finds an export by its name. Only the marked overload is refused.
#[rstest]
fn invalid_export_on_an_overloaded_function(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Twice : INT
VAR_INPUT a : INT; END_VAR
    Twice := a * 2;
END_FUNCTION

FUNCTION Twice : REAL
VAR_INPUT a : REAL; END_VAR
    Twice := a * 2.0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'Twice' cannot be exported: it is overloaded
       |
       | Note: the module finds an export by its name, which several FUNCTIONs share
    ---'
    ");
}

/// A FUNCTION exported under a name the module exports itself would make a
/// module with two exports of that name, which does not load.
#[rstest]
fn export_named_like_a_module_export_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION memory : INT
    memory := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'memory' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// `cabi_realloc` is what a host places a list with when it calls the
/// control interface: a FUNCTION exported under that name would be a second
/// export of it.
#[rstest]
fn export_named_like_the_list_allocator_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION cabi_realloc : INT
    cabi_realloc := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'cabi_realloc' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// A `{test}` is exported under its name in a debug build.
#[rstest]
fn test_named_like_a_module_export_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION __init
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- '__init' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// In a namespace the export is `Io.memory`, which the module does not use.
#[rstest]
fn export_in_a_namespace_may_take_a_module_export_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Io
    {export}
    FUNCTION memory : INT
        memory := 1;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A pragma rk has no rule for is E1510 wherever it stands: above a POU,
/// in a declaration section, after a type, in a body. Inside a section it
/// used to be read and dropped without a word. It is read to its `}`, so a
/// `//` or a `}` inside a quoted string is text: the `//` used to start a
/// line comment that ate the `}`. A known name in capitals is pointed at
/// its lowercase spelling. A `{` in a string, a comment or a known pragma
/// opens no pragma, and one inside a pragma's text opens no second one.
#[rstest]
fn invalid_unknown_pragmas(mut with_db: RootDatabase) {
    let source = r#"
{attribute 'qualified_only'}
FUNCTION_BLOCK Fb
VAR
    {attribute 'see http://x'}
    a : INT;
    b : INT {attribute 'a}b'};
END_VAR
    {pack_mode := '1'}
    a := 1;
END_FUNCTION_BLOCK

{TEST}
FUNCTION t
END_FUNCTION

{info = 'a {braced} note'}
FUNCTION u : STRING
    u := '{not a pragma}'; (* {nor this} *) // {nor this}
    {attribute '{u}'}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {attribute 'qualified_only'}
       | ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |               `--------------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     {attribute 'see http://x'}
       |     ^^^^^^^^^^^^^|^^^^^^^^^^^^
       |                  `-------------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     b : INT {attribute 'a}b'};
       |             ^^^^^^^^|^^^^^^^^
       |                     `---------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:9:5 ]
       |
     9 |     {pack_mode := '1'}
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- no pragma named 'pack_mode'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
        ,-[ file:///test0.st:13:1 ]
        |
     13 | {TEST}
        | ^^^|^^
        |    `---- no pragma named 'TEST'
        |
        | Help: write it in lowercase, {test}
        |
        | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ----'
    [E1510] Error: unknown pragma
        ,-[ file:///test0.st:20:5 ]
        |
     20 |     {attribute '{u}'}
        |     ^^^^^^^^|^^^^^^^^
        |             `---------- no pragma named 'attribute'
        |
        | Help: remove it
        |
        | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ----'
    ");
}

/// A known pragma where its rule cannot stand is a syntax error: `{export}`
/// after a type, and `{ test }`, which `{test}` is written without spaces.
/// Both were read as an unknown pragma and dropped, so the FUNCTION was
/// quietly no test.
#[rstest]
fn invalid_known_pragma_out_of_place(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Row : ARRAY[0..1] OF INT {export};
END_TYPE

{ test }
FUNCTION t
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax error
       ,-[ file:///test0.st:3:30 ]
       |
     3 |     Row : ARRAY[0..1] OF INT {export};
       |                              ^^^^|^^^
       |                                  `----- unexpected token(s): '{export}'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:6:1 ]
       |
     6 | { test }
       | ^^^^|^^^
       |     `----- unexpected token(s): '{ test }'
    ---'
    ");
}

/// A variadic parameter has one copy per argument count, and an import has one
/// signature: this passed `rk check`, and the build looked for a copy.
#[rstest]
fn invalid_extern_variadic(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'sum'}
FUNCTION sum : INT
VAR_INPUT values : INT...; END_VAR
END_FUNCTION

PROGRAM Main
VAR x : INT; END_VAR
    x := sum(1, 2);
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an import
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT values : INT...; END_VAR
       |           ^^^|^^
       |              `---- VAR_INPUT 'values' cannot cross a WASM import: a variadic parameter has the argument count of each call
       |
       | Note: a WASM import has one signature
    ---'
    ");
}

/// The runner calls a test with no arguments, and its wrapper has no parameters
/// to read. A scalar read a scratch local, what goes by address wrote near address
/// 0, a REAL made the module invalid, and a test taking a variadic or an interface
/// had no copy, so it never ran.
#[rstest]
fn invalid_test_with_a_parameter(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE Itf
    METHOD Run : INT
    END_METHOD
END_INTERFACE

{test}
FUNCTION scalar
VAR_INPUT a : INT; END_VAR
END_FUNCTION

{test}
FUNCTION float
VAR_INPUT a : REAL; END_VAR
END_FUNCTION

{test}
FUNCTION by_address
VAR_IN_OUT a : INT; END_VAR
END_FUNCTION

{test}
FUNCTION output
VAR_OUTPUT a : REAL; END_VAR
END_FUNCTION

{test}
FUNCTION pack
VAR_INPUT a : INT...; END_VAR
END_FUNCTION

{test}
FUNCTION implementation
VAR_INPUT a : Itf; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1511] Error: test with a parameter
       ,-[ file:///test0.st:9:11 ]
       |
     9 | VAR_INPUT a : INT; END_VAR
       |           |
       |           `-- the {test} FUNCTION cannot take the parameter 'a'
       |
       | Help: declare it in VAR, or call a FUNCTION taking it from the test
       |
       | Note: the runner calls a test with no arguments
    ---'
    [E1511] Error: test with a parameter
        ,-[ file:///test0.st:14:11 ]
        |
     14 | VAR_INPUT a : REAL; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test with a parameter
        ,-[ file:///test0.st:19:12 ]
        |
     19 | VAR_IN_OUT a : INT; END_VAR
        |            |
        |            `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test with a parameter
        ,-[ file:///test0.st:24:12 ]
        |
     24 | VAR_OUTPUT a : REAL; END_VAR
        |            |
        |            `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test with a parameter
        ,-[ file:///test0.st:29:11 ]
        |
     29 | VAR_INPUT a : INT...; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test with a parameter
        ,-[ file:///test0.st:34:11 ]
        |
     34 | VAR_INPUT a : Itf; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    ");
}

/// A result is a local of the test, which nothing reads.
#[rstest]
fn valid_test_with_a_result(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT a : INT; b : REAL; END_STRUCT; END_TYPE

{test}
FUNCTION float : REAL
    float := 1.0;
END_FUNCTION

{test}
FUNCTION aggregate : Pair
    aggregate.b := 1.0;
END_FUNCTION

{test}
FUNCTION text : STRING
    text := 'a';
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
