// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How WIT names are written in ST: a namespace in words that start with a
//! capital, a POU, a type or an enum's value in capitals joined by `_`, a
//! variable in lowercase; a keyword of ST takes a `_` after it, wherever it
//! stands. The names WIT gives come first: of two written alike in one
//! namespace the later function is left out, a name the generator makes
//! takes a `_`, and so does a variable that would be its POU's name or one
//! the generated code uses there. Two variables WIT names alike in one POU
//! leave the function out, and two interfaces of one namespace or one file
//! stop the generator.

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

    			/* Lets go of `handle`, a handle of `task` the host gave. */
    			{extern 'wasi:example/exit@0.3.0' '[resource-drop]task'}
    			FUNCTION TASK_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/exit@0.3.0' '[method]task.step'}
    			FUNCTION TASK_STEP : TIME_
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/exit@0.3.0' 'exit'}
    			FUNCTION EXIT_
    			VAR_INPUT
    				by_ : ANSWER;
    				at_ : TIME_;
    			END_VAR
    			VAR_OUTPUT
    				step_ : UDINT;
    				of_ : UDINT;
    			END_VAR
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
    wasi-example-clash.st: X_DROP, Y_DROP, X_NEW, Y_TURN, KEPT
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
    "#), @"wasi-example-clash.st: B_DROP_, SHAPE, B_DROP, OPEN_RAW_, OPEN, OPEN_RAW");
}

/// A variable that would be the name of its FUNCTION's value, or a name
/// the generated code uses in its POU, takes a `_` after it, as a keyword
/// does: a parameter named as its function, by itself or once its record's
/// field is added; a block's field `done` beside its `DONE`; a parameter
/// `a0` beside the word a variant is packed into, and one named as the
/// import that takes the words; a parameter `w0` beside the word a result
/// is read out of; a parameter `call` of a block that holds its import's.
/// Where nothing has the name, the variable keeps it:
/// `status` gives a `done`, `plain` takes an `a0`, a `w0` and a `call`,
/// and `tally`, which returns no value of its own, gives a `tally`.
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
            run: async func() -> finished;
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

    			TYPE SHAPE : USINT (
    				NONE := 0,
    				CIRCLE := 1
    			); END_TYPE

    			{extern 'wasi:example/clash@0.3.0' 'f'}
    			FUNCTION F : UDINT
    			VAR_INPUT
    				f_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'get-x'}
    			FUNCTION GET_X : UDINT
    			VAR_INPUT
    				get_x_ : UDINT;
    				get_y : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'exit'}
    			FUNCTION EXIT_ : UDINT
    			VAR_INPUT
    				exit__ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern async 'wasi:example/clash@0.3.0' 'run'}
    			FUNCTION_BLOCK RUN
    			VAR_OUTPUT
    				DONE : BOOL;
    				done_ : BOOL;
    			END_VAR
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
    				target_circle : REAL := 0.0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%B0 := USINT_TO_BYTE(target);
    				CASE target OF
    					SHAPE#CIRCLE:
    						a1 := target_circle;
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
    				target_circle : REAL := 0.0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%B0 := USINT_TO_BYTE(target);
    				CASE target OF
    					SHAPE#CIRCLE:
    						a1 := target_circle;
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

    			FUNCTION PROBE
    			VAR_INPUT
    				w0_ : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				value : SHAPE;
    				circle : REAL;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				PROBE_RAW(w0_ := w0_, w0 => w0, w1 => w1);
    				circle := 0.0;
    				value := BYTE_TO_USINT(w0.%B0);
    				CASE value OF
    					SHAPE#CIRCLE:
    						circle := DWORD_TO_REAL(w0.%D1);
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
    				circle : REAL;
    			END_VAR
    			VAR
    				call : LATER_RAW;
    			END_VAR
    				call(call_ := call_);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					circle := 0.0;
    					value := BYTE_TO_USINT(call.w0.%B0);
    					CASE value OF
    						SHAPE#CIRCLE:
    							circle := DWORD_TO_REAL(call.w0.%D1);
    					END_CASE;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'wasi:example/clash@0.3.0' 'status'}
    			FUNCTION STATUS
    			VAR_OUTPUT
    				done : BOOL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'plain'}
    			FUNCTION PLAIN
    			VAR_INPUT
    				a0 : UDINT;
    				w0 : UDINT;
    				call : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/clash@0.3.0' 'tally'}
    			FUNCTION TALLY
    			VAR_OUTPUT
    				tally : UDINT;
    				total : UDINT;
    			END_VAR
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
