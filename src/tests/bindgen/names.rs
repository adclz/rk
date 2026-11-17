// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How WIT names are written in ST: a namespace in words that start with a
//! capital, a POU, a type or an enum's value in capitals joined by `_`, a
//! variable or a member in lowercase; a keyword of ST takes a `_` after it,
//! wherever it stands. The names WIT gives come first: of two written alike
//! in one namespace the later function is left out, a function named as a
//! type and a name the generator makes take a `_`, and so does a variable
//! that would be its POU's name or one the generated code uses there. Two
//! variables WIT names alike in one POU leave the function out, two members
//! alike leave their type out, and two interfaces of one namespace or one
//! file stop the generator.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{assert_abi, built, checked, generated, summary};
use crate::tests::utils::with_db;

/// What rk-bindgen declares for `wit`, by name, and what it leaves out,
/// once rk has taken the code and every import has its core type.
fn declared(db: &mut RootDatabase, wit: &str) -> String {
    let bindings = generated(wit);
    let (mir, module) = built(db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    summary(&bindings.files)
}

/// Words joined by `-` in each place a name stands: a package, an interface,
/// a function, a parameter, a type, an enum's value.
#[rstest]
fn words_in_each_place(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:many-words@0.3.0;

        interface monotonic-clock-reader {
            enum read-mode { as-is, rounded-down }
            type tick-count = u64;
            read-the-clock: func(read-mode: read-mode, round-to-nearest: u32) -> tick-count;
        }

        world example {
            import monotonic-clock-reader;
        }
    "#), @r"
    // wasi-many-words-monotonic-clock-reader.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE ManyWords
    		NAMESPACE MonotonicClockReader
    			TYPE READ_MODE : USINT (
    				AS_IS := 0,
    				ROUNDED_DOWN := 1
    			); END_TYPE

    			TYPE TICK_COUNT : ULINT; END_TYPE

    			{extern 'wasi:many-words/monotonic-clock-reader@0.3.0' 'read-the-clock'}
    			FUNCTION READ_THE_CLOCK : TICK_COUNT
    			VAR_INPUT
    				read_mode : READ_MODE;
    				round_to_nearest : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A keyword of ST, in any place: the interface `exit`, the function
/// `exit`, a parameter `by`, an enum's values `true` and `do`, a type
/// `time`, a resource `task`, outputs `step` and `of`.
#[rstest]
fn a_keyword_takes_an_underscore_wherever_it_stands(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface exit {
            enum answer { true, do, maybe }
            type time = u64;
            resource task {
                step: func() -> time;
            }
            record point { step: u32, of: u32 }
            exit: func(by: answer, at: time) -> point;
        }

        world example {
            import exit;
        }
    "#), @r"
    // wasi-example-exit.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Exit_
    			TYPE ANSWER : USINT (
    				TRUE_ := 0,
    				DO_ := 1,
    				MAYBE := 2
    			); END_TYPE

    			TYPE TIME_ : ULINT; END_TYPE

    			TYPE TASK_ : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `task` the host gave. */
    			{extern 'wasi:example/exit@0.3.0' '[resource-drop]task'}
    			FUNCTION TASK_DROP
    			VAR_INPUT
    				handle : TASK_;
    			END_VAR
    			END_FUNCTION

    			TYPE POINT : STRUCT
    				step_ : UDINT;
    				of_ : UDINT;
    			END_STRUCT END_TYPE

    			{extern 'wasi:example/exit@0.3.0' '[method]task.step'}
    			FUNCTION TASK_STEP : TIME_
    			VAR_INPUT
    				self : TASK_;
    			END_VAR
    			END_FUNCTION

    			/* `exit` as the host gives it: the values of its result, which `EXIT_`
    			   puts in its members. */
    			{extern 'wasi:example/exit@0.3.0' 'exit'}
    			FUNCTION PRIVATE EXIT__RAW
    			VAR_INPUT
    				by_ : ANSWER;
    				at_ : TIME_;
    			END_VAR
    			VAR_OUTPUT
    				step_ : UDINT;
    				of_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION EXIT_ : POINT
    			VAR_INPUT
    				by_ : ANSWER;
    				at_ : TIME_;
    			END_VAR
    				EXIT__RAW(by_ := by_, at_ := at_, step_ => EXIT_.step_, of_ => EXIT_.of_);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Two WIT names that ST writes alike in one namespace: the function
/// `x-new` and the constructor of `x`, the function `y-turn` and the method
/// `turn` of `y`. The later one is left out, and the file says which name
/// it would have had.
#[rstest]
fn two_names_written_alike_leave_the_later_out(mut with_db: RootDatabase) {
    assert_snapshot!(declared(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            resource x {
                constructor();
            }
            x-new: func() -> u32;
            y-turn: func() -> u32;
            resource y {
                turn: func();
            }
            kept: func() -> u32;
        }

        world example {
            import clash;
        }
    "#), @r"
    wasi-example-clash.st: X, X_DROP, Y, Y_DROP, X_NEW, Y_TURN, KEPT
        left out `x-new`: it would be `X_NEW`, which `[constructor]x` is already
        left out `[method]y.turn`: it would be `Y_TURN`, which `y-turn` is already
    ");
}

/// A name the generator makes gives way to one WIT has, with a `_` after
/// it: the drop of `b` beside its method `drop`, and the import of `open`,
/// whose result is read out of words, beside the function `open-raw`. A
/// built-in does the same (`built_ins`).
#[rstest]
fn a_name_the_generator_makes_gives_way(mut with_db: RootDatabase) {
    assert_snapshot!(declared(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            resource b {
                drop: func();
            }
            variant shape { none, circle(f32) }
            open: func() -> shape;
            open-raw: func() -> u32;
        }

        world example {
            import clash;
        }
    "#), @"wasi-example-clash.st: B, B_DROP_, SHAPE_KIND, SHAPE, B_DROP, OPEN_RAW_, OPEN, OPEN_RAW");
}

/// A function named as a type takes a `_` after its name, which a program
/// gives the type: the method `stat` of `file` beside the record
/// `file-stat`, `shape-kind` beside the enum of the cases of `shape`, and
/// the method `y` of `x` beside the resource `x-y`.
#[rstest]
fn a_function_named_as_a_type_takes_an_underscore(mut with_db: RootDatabase) {
    assert_snapshot!(declared(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            record file-stat { size: u64 }
            resource file {
                stat: func() -> file-stat;
            }
            variant shape { none, circle(f32) }
            shape-kind: func() -> u32;
            resource x {
                y: func();
            }
            resource x-y;
        }

        world example {
            import clash;
        }
    "#), @"wasi-example-clash.st: FILE_STAT, FILE, FILE_DROP, SHAPE_KIND, SHAPE, X, X_DROP, X_Y, X_Y_DROP, FILE_STAT__RAW, FILE_STAT_, SHAPE_KIND_, X_Y_");
}

/// A variable that would be the name of its FUNCTION's value, or a name
/// the generated code uses in its POU, takes a `_` after it, as a keyword
/// does: a parameter named as its function; a block's parameter `done`
/// beside its `DONE`; a parameter `a0` beside the word a variant is packed
/// into, and one named as the import that takes the words; a parameter `w0`
/// beside the word a result is read out of; a parameter `call` of a block
/// that holds its import's. Where nothing has the name, the variable keeps
/// it: `plain` takes an `a0`, a `w0` and a `call`. A member of a STRUCT
/// keeps its name whatever the POU's: a record passed whole is one input,
/// `get`, whose fields its import takes as `get_x` and `get_y`, and the
/// `done` of the record `run` gives, the `done` of `status`'s and the
/// `tally` of `tally`'s are members of the value each returns.
#[rstest]
fn a_variable_keeps_away_from_the_names_its_pou_has(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            record point { x: u32, y: u32 }
            record finished { done: bool }
            record counted { tally: u32, total: u32 }
            variant shape { none, circle(f32) }
            f: func(f: u32) -> u32;
            get-x: func(get: point) -> u32;
            exit: func(exit: u32) -> u32;
            run: async func(done: u32) -> finished;
            aim: func(a0: u32, target: shape);
            scan: func(scan-raw: u32, target: shape) -> u32;
            probe: func(w0: u32) -> shape;
            later: async func(call: u32) -> shape;
            status: func() -> finished;
            plain: func(a0: u32, w0: u32, call: u32);
            tally: func() -> counted;
        }

        world example {
            import clash;
        }
    "#), @r"
    // wasi-example-clash.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clash
    			USING Std.Convert;

    			TYPE POINT : STRUCT
    				x : UDINT;
    				y : UDINT;
    			END_STRUCT END_TYPE

    			TYPE FINISHED : STRUCT
    				done : BOOL;
    			END_STRUCT END_TYPE

    			TYPE COUNTED : STRUCT
    				tally : UDINT;
    				total : UDINT;
    			END_STRUCT END_TYPE

    			/* The cases of `shape`. */
    			TYPE SHAPE_KIND : USINT (
    				NONE := 0,
    				CIRCLE := 1
    			); END_TYPE

    			TYPE SHAPE : STRUCT
    				kind : SHAPE_KIND;
    				circle : REAL;
    			END_STRUCT END_TYPE

    			{extern 'wasi:example/clash@0.3.0' 'f'}
    			FUNCTION F : UDINT
    			VAR_INPUT
    				f_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* `get-x` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `GET_X` passes field by field. */
    			{extern 'wasi:example/clash@0.3.0' 'get-x'}
    			FUNCTION PRIVATE GET_X_RAW : UDINT
    			VAR_INPUT
    				get_x : UDINT;
    				get_y : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION GET_X : UDINT
    			VAR_INPUT
    				get : POINT;
    			END_VAR
    				GET_X := GET_X_RAW(get_x := get.x, get_y := get.y);
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'exit'}
    			FUNCTION EXIT_ : UDINT
    			VAR_INPUT
    				exit__ : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* `run` as the host gives it: the one value of its result, which `RUN`
    			   puts in the member of `value`. */
    			{extern async 'wasi:example/clash@0.3.0' 'run'}
    			FUNCTION_BLOCK RUN_RAW
    			VAR_INPUT
    				done_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				done__ : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK RUN
    			VAR_INPUT
    				done_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : FINISHED;
    			END_VAR
    			VAR
    				call : RUN_RAW;
    			END_VAR
    				call(done_ := done_);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.done := call.done__;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* `aim` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `AIM` packs. */
    			{extern 'wasi:example/clash@0.3.0' 'aim'}
    			FUNCTION PRIVATE AIM_RAW
    			VAR_INPUT
    				a0_ : UDINT;
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION AIM
    			VAR_INPUT
    				a0_ : UDINT;
    				target : SHAPE;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%B0 := USINT_TO_BYTE(target.kind);
    				CASE target.kind OF
    					SHAPE_KIND#CIRCLE:
    						a1 := target.circle;
    				END_CASE;
    				AIM_RAW(a0_ := a0_, a0 := a0, a1 := a1);
    			END_FUNCTION

    			/* `scan` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `SCAN` packs. */
    			{extern 'wasi:example/clash@0.3.0' 'scan'}
    			FUNCTION PRIVATE SCAN_RAW : UDINT
    			VAR_INPUT
    				scan_raw_ : UDINT;
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION SCAN : UDINT
    			VAR_INPUT
    				scan_raw_ : UDINT;
    				target : SHAPE;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%B0 := USINT_TO_BYTE(target.kind);
    				CASE target.kind OF
    					SHAPE_KIND#CIRCLE:
    						a1 := target.circle;
    				END_CASE;
    				SCAN := SCAN_RAW(scan_raw_ := scan_raw_, a0 := a0, a1 := a1);
    			END_FUNCTION

    			/* `probe` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `PROBE` reads. */
    			{extern 'wasi:example/clash@0.3.0' 'probe'}
    			FUNCTION PRIVATE PROBE_RAW
    			VAR_INPUT
    				w0_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION PROBE : SHAPE
    			VAR_INPUT
    				w0_ : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				PROBE_RAW(w0_ := w0_, w0 => w0, w1 => w1);
    				PROBE.circle := 0.0;
    				PROBE.kind := BYTE_TO_USINT(w0.%B0);
    				CASE PROBE.kind OF
    					SHAPE_KIND#CIRCLE:
    						PROBE.circle := DWORD_TO_REAL(w0.%D1);
    				END_CASE;
    			END_FUNCTION

    			/* `later` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `LATER` reads. */
    			{extern async 'wasi:example/clash@0.3.0' 'later'}
    			FUNCTION_BLOCK LATER_RAW
    			VAR_INPUT
    				call_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				w0 : LWORD;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK LATER
    			VAR_INPUT
    				call_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : SHAPE;
    			END_VAR
    			VAR
    				call : LATER_RAW;
    			END_VAR
    				call(call_ := call_);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.circle := 0.0;
    					value.kind := BYTE_TO_USINT(call.w0.%B0);
    					CASE value.kind OF
    						SHAPE_KIND#CIRCLE:
    							value.circle := DWORD_TO_REAL(call.w0.%D1);
    					END_CASE;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* `status` as the host gives it: the one value of its result, which
    			   `STATUS` puts in its member. */
    			{extern 'wasi:example/clash@0.3.0' 'status'}
    			FUNCTION PRIVATE STATUS_RAW
    			VAR_OUTPUT
    				done : BOOL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION STATUS : FINISHED
    				STATUS_RAW(done => STATUS.done);
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'plain'}
    			FUNCTION PLAIN
    			VAR_INPUT
    				a0 : UDINT;
    				w0 : UDINT;
    				call : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* `tally` as the host gives it: the values of its result, which `TALLY`
    			   puts in its members. */
    			{extern 'wasi:example/clash@0.3.0' 'tally'}
    			FUNCTION PRIVATE TALLY_RAW
    			VAR_OUTPUT
    				tally : UDINT;
    				total : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION TALLY : COUNTED
    				TALLY_RAW(tally => TALLY.tally, total => TALLY.total);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Two interfaces whose namespaces ST takes for one, `foo-bar` and
/// `foobar`: the generator stops, saying which.
#[test]
fn two_interfaces_of_one_namespace_stop_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        interface foo-bar {
            a: func();
        }

        interface foobar {
            b: func();
        }

        world example {
            import foo-bar;
            import foobar;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"`wasi:example/foo-bar@0.3.0` and `wasi:example/foobar@0.3.0` would both be the namespace `Wasi.Example.Foobar`, which ST takes for one");
}

/// Two versions of one interface are one namespace too.
#[test]
fn two_versions_of_an_interface_stop_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        package wasi:dep@0.1.0 {
            interface a {
                f: func();
            }
        }

        package wasi:dep@0.2.0 {
            interface a {
                f: func();
                g: func();
            }
        }

        world example {
            import wasi:dep/a@0.1.0;
            import wasi:dep/a@0.2.0;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"`wasi:dep/a@0.1.0` and `wasi:dep/a@0.2.0` would both be the namespace `Wasi.Dep.A`, which ST takes for one");
}

/// Two interfaces of two namespaces whose files would have one name: the
/// package `foo-bar` with `baz`, and the package `foo` with `bar-baz`.
#[test]
fn two_interfaces_of_one_file_stop_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        package wasi:foo-bar@0.1.0 {
            interface baz {
                f: func();
            }
        }

        package wasi:foo@0.1.0 {
            interface bar-baz {
                f: func();
            }
        }

        world example {
            import wasi:foo-bar/baz@0.1.0;
            import wasi:foo/bar-baz@0.1.0;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"`wasi:foo-bar/baz@0.1.0` and `wasi:foo/bar-baz@0.1.0` would both be the file `wasi-foo-bar-baz.st`");
}

/// Two variables of one POU that ST writes alike: a parameter `value` and
/// the option's `value` that comes back, a record parameter whose field
/// meets another parameter. The function is left out, saying why.
#[rstest]
fn two_variables_written_alike_leave_the_function_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            record p { x: u32 }
            lookup: func(value: u32) -> option<u32>;
            place: func(p: p, p-x: u32);
            fine: func(value: u32) -> u32;
        }

        world example {
            import clash;
        }
    "#), @r"
    // wasi-example-clash.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clash
    			TYPE P_ : STRUCT
    				x : UDINT;
    			END_STRUCT END_TYPE

    			{extern 'wasi:example/clash@0.3.0' 'fine'}
    			FUNCTION FINE : UDINT
    			VAR_INPUT
    				value : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `lookup`: two of its variables would be `value`, which ST takes for
         one.
       - `place`: two of its variables would be `p_x`, which ST takes for one. */
    ");
}

/// An output of an import named after a member of the STRUCT that comes
/// back takes a `_` while a variable beside it has the name: `moved`'s
/// `spot` beside the input `spot` of `move-to`, `held`'s `value` beside
/// the output `value` of the block `fetch`.
#[rstest]
fn an_output_of_the_import_keeps_clear_of_the_variables_beside_it(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            record point { x: u32, y: u32 }
            record moved { spot: u32, far: bool }
            record held { value: u32, n: u32 }
            move-to: func(spot: point) -> moved;
            fetch: async func() -> held;
        }

        world example {
            import clash;
        }
    "#), @r"
    // wasi-example-clash.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clash
    			TYPE POINT : STRUCT
    				x : UDINT;
    				y : UDINT;
    			END_STRUCT END_TYPE

    			TYPE MOVED : STRUCT
    				spot : UDINT;
    				far : BOOL;
    			END_STRUCT END_TYPE

    			TYPE HELD : STRUCT
    				value : UDINT;
    				n_ : UDINT;
    			END_STRUCT END_TYPE

    			/* `move-to` as the host has it: its parameters as the canonical ABI passes
    			   them, which `MOVE_TO` passes field by field, and the values of its
    			   result, which it puts in its members. */
    			{extern 'wasi:example/clash@0.3.0' 'move-to'}
    			FUNCTION PRIVATE MOVE_TO_RAW
    			VAR_INPUT
    				spot_x : UDINT;
    				spot_y : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				spot_ : UDINT;
    				far : BOOL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MOVE_TO : MOVED
    			VAR_INPUT
    				spot : POINT;
    			END_VAR
    				MOVE_TO_RAW(spot_x := spot.x, spot_y := spot.y, spot_ => MOVE_TO.spot, far => MOVE_TO.far);
    			END_FUNCTION

    			/* `fetch` as the host gives it: the values of its result, which `FETCH`
    			   puts in the members of `value`. */
    			{extern async 'wasi:example/clash@0.3.0' 'fetch'}
    			FUNCTION_BLOCK FETCH_RAW
    			VAR_OUTPUT
    				DONE : BOOL;
    				value_ : UDINT;
    				n_ : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK FETCH
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : HELD;
    			END_VAR
    			VAR
    				call : FETCH_RAW;
    			END_VAR
    				call();
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.value := call.value_;
    					value.n_ := call.n_;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Two members of one STRUCT that ST writes alike: a case `kind` beside
/// the member that holds the case, the flag of an option `x` beside a
/// field `x-is-some`, and the first item of a tuple `a` beside a field
/// `a-0`. The type is left out, saying why, and so are an alias of it, a
/// STRUCT that holds it, and the functions that take or give it, one that
/// would pass it field by field among them.
#[rstest]
fn two_members_written_alike_leave_the_type_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            variant token { kind(u8), other }
            record flagged { x: option<u32>, x-is-some: bool }
            type same = token;
            record holder { t: token, n: u32 }
            record spread { a: tuple<u32, u32>, a-0: u32 }
            next: func() -> token;
            put: func(t: token);
            flag: func() -> flagged;
            hold: func(h: holder);
            again: func() -> same;
            send: func(s: spread);
            fine: func() -> u32;
        }

        world example {
            import clash;
        }
    "#), @r"
    // wasi-example-clash.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clash
    			{extern 'wasi:example/clash@0.3.0' 'fine'}
    			FUNCTION FINE : UDINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `token`: two of its members would be `kind`, which ST takes for one.
       - `flagged`: two of its members would be `x_is_some`, which ST takes for
         one.
       - `same`: it names `token`, which is left out.
       - `holder`: it holds `token`, which is left out.
       - `spread`: two of its members would be `a_0`, which ST takes for one.
       - `next`: its result holds a value whose STRUCT is left out, which an
         import cannot give back yet.
       - `put`: its parameter `t` holds a value whose STRUCT is left out, which
         an import cannot take yet.
       - `flag`: its result holds a value whose STRUCT is left out, which an
         import cannot give back yet.
       - `hold`: its parameter `h` holds a value whose STRUCT is left out,
         which an import cannot take yet.
       - `again`: its result holds a value whose STRUCT is left out, which an
         import cannot give back yet.
       - `send`: its parameter `s` holds a value whose STRUCT is left out,
         which an import cannot take yet. */
    ");
}

/// Every keyword of ST's grammar that a WIT name could become is one the
/// generated names keep away from.
#[test]
fn the_keywords_are_the_grammar_s() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/tree-sitter/src/node-types.json"
    );
    let nodes: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the grammar's node types"))
            .expect("JSON");
    let mut words = std::collections::BTreeSet::new();
    fn walk(value: &serde_json::Value, words: &mut std::collections::BTreeSet<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if let (
                    Some(serde_json::Value::String(word)),
                    Some(serde_json::Value::Bool(false)),
                ) = (object.get("type"), object.get("named"))
                {
                    let wit_like = word.starts_with(|c: char| c.is_ascii_alphabetic())
                        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                    if wit_like {
                        words.insert(word.clone());
                    }
                }
                object.values().for_each(|value| walk(value, words));
            }
            serde_json::Value::Array(values) => values.iter().for_each(|value| walk(value, words)),
            _ => {}
        }
    }
    walk(&nodes, &mut words);
    let missing: Vec<&String> = words
        .iter()
        .filter(|word| {
            !rk_bindgen::names::KEYWORDS
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(word))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "keywords the generator does not keep away from: {missing:?}"
    );
}
