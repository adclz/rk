// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Options and results that come back: an option is its flag `is_some`
//! then its `value`, a result its flag `failed` then its ok `value` or its
//! `error`, laid out as the canonical ABI lays the variant out. A result
//! with both, whose value and error share their place, and an option of a
//! record come back in words, read out of them. Either passed in is its
//! flag and what it holds, packed into the values the canonical ABI passes
//! it as.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::component::{Linker, Val};

use super::{checked, hosted};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// An option of each kind of scalar: a number, an enum, an alias, a handle.
#[rstest]
fn an_option_is_a_flag_and_a_value(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface finder {
            enum kind { file, folder }
            type size = u64;
            resource entry;
            offset: func() -> option<s64>;
            kind-of: func() -> option<kind>;
            size-of: func() -> option<size>;
            first: func() -> option<entry>;
            flag: func() -> option<bool>;
        }

        world example {
            import finder;
        }
    "#), @r"
    // wasi-example-finder.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Finder
    			TYPE KIND : USINT (
    				FILE := 0,
    				FOLDER := 1
    			); END_TYPE

    			TYPE SIZE : ULINT; END_TYPE

    			TYPE ENTRY : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `entry` the host gave. */
    			{extern 'wasi:example/finder@0.3.0' '[resource-drop]entry'}
    			FUNCTION ENTRY_DROP
    			VAR_INPUT
    				handle : ENTRY;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/finder@0.3.0' 'offset'}
    			FUNCTION OFFSET
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : LINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/finder@0.3.0' 'kind-of'}
    			FUNCTION KIND_OF
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : KIND;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/finder@0.3.0' 'size-of'}
    			FUNCTION SIZE_OF
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : SIZE;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/finder@0.3.0' 'first'}
    			FUNCTION FIRST
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : ENTRY;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/finder@0.3.0' 'flag'}
    			FUNCTION FLAG
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : BOOL;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A result of nothing is its flag; with an ok value or an error, the flag
/// then that.
#[rstest]
fn a_result_is_a_flag_and_one_side(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface outcomes {
            enum error-code { busy, gone }
            resource handle;
            done: func() -> result;
            count: func() -> result<u32>;
            open: func() -> result<handle>;
            check: func() -> result<_, error-code>;
            code: func() -> result<_, u16>;
        }

        world example {
            import outcomes;
        }
    "#), @r"
    // wasi-example-outcomes.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Outcomes
    			TYPE ERROR_CODE : USINT (
    				BUSY := 0,
    				GONE := 1
    			); END_TYPE

    			TYPE HANDLE : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `handle` the host gave. */
    			{extern 'wasi:example/outcomes@0.3.0' '[resource-drop]handle'}
    			FUNCTION HANDLE_DROP
    			VAR_INPUT
    				handle : HANDLE;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/outcomes@0.3.0' 'done'}
    			FUNCTION DONE
    			VAR_OUTPUT
    				failed : BOOL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/outcomes@0.3.0' 'count'}
    			FUNCTION COUNT
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/outcomes@0.3.0' 'open'}
    			FUNCTION OPEN
    			VAR_OUTPUT
    				failed : BOOL;
    				value : HANDLE;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/outcomes@0.3.0' 'check'}
    			FUNCTION CHECK
    			VAR_OUTPUT
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/outcomes@0.3.0' 'code'}
    			FUNCTION CODE
    			VAR_OUTPUT
    				failed : BOOL;
    				error : UINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What two outputs are not laid out as comes back in words: a result
/// with a value and an error, which share their place in memory, and an
/// option of a record. The side or the value that did not come is zero.
#[rstest]
fn what_two_outputs_cannot_hold_comes_back_in_words(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface both-sides {
            enum error-code { busy, gone }
            record point { x: f64, y: f64 }
            both: func() -> result<u32, error-code>;
            where-is: func() -> option<point>;
            deep: func() -> result<option<point>, error-code>;
        }

        world example {
            import both-sides;
        }
    "#), @r"
    // wasi-example-both-sides.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE BothSides
    			USING Std.Convert;

    			TYPE ERROR_CODE : USINT (
    				BUSY := 0,
    				GONE := 1
    			); END_TYPE

    			TYPE POINT : STRUCT
    				x : LREAL;
    				y : LREAL;
    			END_STRUCT END_TYPE

    			/* `both` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `BOTH` reads. */
    			{extern 'wasi:example/both-sides@0.3.0' 'both'}
    			FUNCTION PRIVATE BOTH_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION BOTH
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				BOTH_RAW(w0 => w0, w1 => w1);
    				value := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value := DWORD_TO_UDINT(w0.%D1);
    				END_IF;
    			END_FUNCTION

    			/* `where-is` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `WHERE_IS` reads. */
    			{extern 'wasi:example/both-sides@0.3.0' 'where-is'}
    			FUNCTION PRIVATE WHERE_IS_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WHERE_IS
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : POINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				WHERE_IS_RAW(w0 => w0, w1 => w1, w2 => w2);
    				value.x := 0.0;
    				value.y := 0.0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value.x := LWORD_TO_LREAL(w1);
    					value.y := LWORD_TO_LREAL(w2);
    				END_IF;
    			END_FUNCTION

    			/* `deep` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `DEEP` reads. */
    			{extern 'wasi:example/both-sides@0.3.0' 'deep'}
    			FUNCTION PRIVATE DEEP_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION DEEP
    			VAR_OUTPUT
    				failed : BOOL;
    				value_is_some : BOOL;
    				value : POINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    				DEEP_RAW(w0 => w0, w1 => w1, w2 => w2, w3 => w3);
    				value_is_some := FALSE;
    				value.x := 0.0;
    				value.y := 0.0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w1.%B0);
    				ELSE
    					value_is_some := w1.%X0;
    					IF value_is_some THEN
    						value.x := LWORD_TO_LREAL(w2);
    						value.y := LWORD_TO_LREAL(w3);
    					END_IF;
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// An option or a result passed in is its flag, then what it holds, which
/// a call may leave out: the import takes them as the values the canonical
/// ABI flattens them into, a REAL or an LREAL where each side is one. A
/// result of a string cannot come back yet.
#[rstest]
fn an_option_or_a_result_passed_in_is_its_flag_and_what_it_holds(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface leftovers {
            enum error-code { busy }
            name: func() -> result<string>;
            maybe-in: func(x: option<u32>);
            result-in: func(x: result<_, error-code>);
            warm-in: func(t: option<f32>);
            far-in: func(d: result<f64, f64>);
            kept: func() -> u8;
        }

        world example {
            import leftovers;
        }
    "#), @r"
    // wasi-example-leftovers.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Leftovers
    			USING Std.Convert;

    			TYPE ERROR_CODE : USINT (
    				BUSY := 0
    			); END_TYPE

    			/* `name` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `NAME` reads, and its string in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/leftovers@0.3.0' 'name'}
    			FUNCTION PRIVATE NAME_RAW
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION NAME
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				NAME_RAW(value := value, w0 => w0, w1 => w1);
    				failed := w0.%X0;
    			END_FUNCTION

    			/* `maybe-in` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `MAYBE_IN` packs. */
    			{extern 'wasi:example/leftovers@0.3.0' 'maybe-in'}
    			FUNCTION PRIVATE MAYBE_IN_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : DWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MAYBE_IN
    			VAR_INPUT
    				x_is_some : BOOL;
    				x : UDINT := 0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : DWORD;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a0.%X0 := x_is_some;
    				IF x_is_some THEN
    					a1 := UDINT_TO_DWORD(x);
    				END_IF;
    				MAYBE_IN_RAW(a0 := a0, a1 := a1);
    			END_FUNCTION

    			/* `result-in` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `RESULT_IN` packs. */
    			{extern 'wasi:example/leftovers@0.3.0' 'result-in'}
    			FUNCTION PRIVATE RESULT_IN_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : DWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION RESULT_IN
    			VAR_INPUT
    				x_failed : BOOL;
    				x_error : ERROR_CODE := 0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : DWORD;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a0.%X0 := x_failed;
    				IF x_failed THEN
    					a1.%B0 := USINT_TO_BYTE(x_error);
    				END_IF;
    				RESULT_IN_RAW(a0 := a0, a1 := a1);
    			END_FUNCTION

    			/* `warm-in` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `WARM_IN` packs. */
    			{extern 'wasi:example/leftovers@0.3.0' 'warm-in'}
    			FUNCTION PRIVATE WARM_IN_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WARM_IN
    			VAR_INPUT
    				t_is_some : BOOL;
    				t : REAL := 0.0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%X0 := t_is_some;
    				IF t_is_some THEN
    					a1 := t;
    				END_IF;
    				WARM_IN_RAW(a0 := a0, a1 := a1);
    			END_FUNCTION

    			/* `far-in` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `FAR_IN` packs. */
    			{extern 'wasi:example/leftovers@0.3.0' 'far-in'}
    			FUNCTION PRIVATE FAR_IN_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : LREAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FAR_IN
    			VAR_INPUT
    				d_failed : BOOL;
    				d_value : LREAL := 0.0;
    				d_error : LREAL := 0.0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : LREAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%X0 := d_failed;
    				IF d_failed THEN
    					a1 := d_error;
    				ELSE
    					a1 := d_value;
    				END_IF;
    				FAR_IN_RAW(a0 := a0, a1 := a1);
    			END_FUNCTION

    			{extern 'wasi:example/leftovers@0.3.0' 'kept'}
    			FUNCTION KEPT : USINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What a host leaves out, and what comes back in one value: an option and
/// a result of each side, twice over, one after the other and in a block;
/// an option an option holds; one value in tuples of one.
const ABSENT: &str = r#"
    package wasi:example@0.3.0;

    interface absent {
        maybe: func(some: bool) -> option<u32>;
        settle: func(fail: bool) -> result<u64>;
        refuse: func(fail: bool) -> result<_, u8>;
        maybe-later: async func(some: bool) -> option<u32>;
        twice: func(x: option<option<u32>>) -> option<option<u32>>;
        nested: func() -> tuple<tuple<tuple<u8>>>;
    }

    world example {
        import absent;
    }
"#;

/// The host's side of [`ABSENT`]: `maybe` gives 41 when asked for a value,
/// `settle` the largest u64 unless it fails, `refuse` the error 9 when it
/// does, `twice` what it took and `nested` 7.
fn absent_host(linker: &mut Linker<Host>) {
    let flag = |params: &[Val]| match params {
        [Val::Bool(flag)] => *flag,
        other => panic!("a bool: {other:?}"),
    };
    let maybe = |some: bool| Val::Option(some.then(|| Box::new(Val::U32(41))));
    let mut absent = linker
        .instance("wasi:example/absent@0.3.0")
        .expect("the interface");
    absent
        .func_new("maybe", move |_, _, params, results| {
            results[0] = maybe(flag(params));
            Ok(())
        })
        .expect("a function of the host");
    absent
        .func_new("settle", move |_, _, params, results| {
            results[0] = Val::Result(match flag(params) {
                true => Err(None),
                false => Ok(Some(Box::new(Val::U64(u64::MAX)))),
            });
            Ok(())
        })
        .expect("a function of the host");
    absent
        .func_new("refuse", move |_, _, params, results| {
            results[0] = Val::Result(match flag(params) {
                true => Err(Some(Box::new(Val::U8(9)))),
                false => Ok(None),
            });
            Ok(())
        })
        .expect("a function of the host");
    absent
        .func_new_concurrent("maybe-later", move |_, _, params, results| {
            Box::pin(async move {
                results[0] = maybe(flag(params));
                Ok(())
            })
        })
        .expect("an async function of the host");
    absent
        .func_new("twice", |_, _, params, results| {
            results[0] = params[0].clone();
            Ok(())
        })
        .expect("a function of the host");
    absent
        .func_new("nested", |_, _, _, results| {
            let one = |value: Val| Val::Tuple(vec![value]);
            results[0] = one(one(one(Val::U8(7))));
            Ok(())
        })
        .expect("a function of the host");
}

/// What the host of [`ABSENT`] gives, read back: 1 when each value came and
/// each one left out is zero, and otherwise the number of the check that
/// failed, negative.
const LEFT_OUT: &str = r#"
    FUNCTION check : DINT
    VAR
        is_some, inner : BOOL;
        failed : BOOL;
        value : UDINT;
        big : ULINT;
        error, item : USINT;
    END_VAR
        Wasi.Example.Absent.MAYBE(some := TRUE, is_some => is_some, value => value);
        IF NOT is_some OR value <> 41 THEN check := -1; RETURN; END_IF;
        // The host writes that no value came, and nothing where one would be.
        Wasi.Example.Absent.MAYBE(some := FALSE, is_some => is_some, value => value);
        IF is_some OR value <> 0 THEN check := -2; RETURN; END_IF;

        Wasi.Example.Absent.SETTLE(fail := FALSE, failed => failed, value => big);
        IF failed OR big <> ULINT#18446744073709551615 THEN check := -3; RETURN; END_IF;
        Wasi.Example.Absent.SETTLE(fail := TRUE, failed => failed, value => big);
        IF NOT failed OR big <> 0 THEN check := -4; RETURN; END_IF;

        Wasi.Example.Absent.REFUSE(fail := TRUE, failed => failed, error => error);
        IF NOT failed OR error <> 9 THEN check := -5; RETURN; END_IF;
        Wasi.Example.Absent.REFUSE(fail := FALSE, failed => failed, error => error);
        IF failed OR error <> 0 THEN check := -6; RETURN; END_IF;

        Wasi.Example.Absent.TWICE(x_is_some := TRUE, x_value_is_some := TRUE,
            x_value := 4000000000, is_some => is_some, value_is_some => inner, value => value);
        IF NOT is_some OR NOT inner OR value <> 4000000000 THEN check := -7; RETURN; END_IF;
        Wasi.Example.Absent.TWICE(x_is_some := TRUE, x_value := 5,
            is_some => is_some, value_is_some => inner, value => value);
        IF NOT is_some OR inner OR value <> 0 THEN check := -8; RETURN; END_IF;
        Wasi.Example.Absent.TWICE(x_is_some := FALSE, x_value_is_some := TRUE, x_value := 5,
            is_some => is_some, value_is_some => inner, value => value);
        IF is_some OR inner OR value <> 0 THEN check := -9; RETURN; END_IF;

        Wasi.Example.Absent.NESTED(item0_0_0 => item);
        IF item <> 7 THEN check := -10; RETURN; END_IF;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR
        checked : BOOL;
        step : INT;
        later : Wasi.Example.Absent.MAYBE_LATER;
    END_VAR
        IF NOT checked THEN
            checked := TRUE;
            output := check();
            IF output <> 0 THEN step := 9; END_IF;
        END_IF;
        CASE step OF
        0:
            later(some := TRUE);
            IF later.DONE THEN
                IF NOT later.is_some OR later.value <> 41 THEN
                    output := -20; step := 9;
                ELSE
                    step := 1;
                END_IF;
            END_IF;
        1:
            later(some := FALSE);
            IF later.DONE THEN
                IF later.is_some OR later.value <> 0 THEN
                    output := -21;
                ELSE
                    output := 1;
                END_IF;
                step := 9;
            END_IF;
        END_CASE;
    END_PROGRAM
"#;

/// A value the host leaves out is zero: the `value` of an option that is
/// none or of a result that failed, the `error` of one that did not, after
/// a call that gave one, in a FUNCTION's outputs and in a block's. An
/// option an option holds comes back as it went, and one value in tuples
/// of one comes back as that value.
#[rstest]
fn what_a_host_leaves_out_is_zero(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, ABSENT, LEFT_OUT, absent_host);
    let mut output = 0;
    for _ in 0..20 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}

/// What [`ABSENT`] is declared as: an option an option holds has its flag
/// after `value`, passed in and given back, and one value in tuples of one
/// is one output, named by its place in each.
#[rstest]
fn an_option_in_an_option_and_one_value_in_tuples(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, ABSENT), @r"
    // wasi-example-absent.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Absent
    			USING Std.Convert;

    			{extern 'wasi:example/absent@0.3.0' 'maybe'}
    			FUNCTION MAYBE
    			VAR_INPUT
    				some : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/absent@0.3.0' 'settle'}
    			FUNCTION SETTLE
    			VAR_INPUT
    				fail : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : ULINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/absent@0.3.0' 'refuse'}
    			FUNCTION REFUSE
    			VAR_INPUT
    				fail : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				error : USINT;
    			END_VAR
    			END_FUNCTION

    			{extern async 'wasi:example/absent@0.3.0' 'maybe-later'}
    			FUNCTION_BLOCK MAYBE_LATER
    			VAR_INPUT
    				some : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				is_some : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* `twice` as the host has it: its parameters as the canonical ABI passes
    			   them, which `TWICE` packs, and its result in words of 8 bytes, which it
    			   reads. */
    			{extern 'wasi:example/absent@0.3.0' 'twice'}
    			FUNCTION PRIVATE TWICE_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : DWORD;
    				a2 : DWORD;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION TWICE
    			VAR_INPUT
    				x_is_some : BOOL;
    				x_value_is_some : BOOL := FALSE;
    				x_value : UDINT := 0;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value_is_some : BOOL;
    				value : UDINT;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : DWORD;
    				a2 : DWORD;
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a2 := 0;
    				a0.%X0 := x_is_some;
    				IF x_is_some THEN
    					a1.%X0 := x_value_is_some;
    					IF x_value_is_some THEN
    						a2 := UDINT_TO_DWORD(x_value);
    					END_IF;
    				END_IF;
    				TWICE_RAW(a0 := a0, a1 := a1, a2 := a2, w0 => w0, w1 => w1);
    				value_is_some := FALSE;
    				value := 0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value_is_some := w0.%X32;
    					IF value_is_some THEN
    						value := DWORD_TO_UDINT(w1.%D0);
    					END_IF;
    				END_IF;
    			END_FUNCTION

    			{extern 'wasi:example/absent@0.3.0' 'nested'}
    			FUNCTION NESTED
    			VAR_OUTPUT
    				item0_0_0 : USINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A flag, and a REAL that every case puts in its place, are packed as
/// they are: the code converts nothing, and its namespace uses no
/// `Std.Convert`.
#[rstest]
fn what_is_packed_as_it_is_converts_nothing(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface plain {
            warm-in: func(t: option<f32>);
        }

        world example {
            import plain;
        }
    "#), @r"
    // wasi-example-plain.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Plain
    			/* `warm-in` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `WARM_IN` packs. */
    			{extern 'wasi:example/plain@0.3.0' 'warm-in'}
    			FUNCTION PRIVATE WARM_IN_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WARM_IN
    			VAR_INPUT
    				t_is_some : BOOL;
    				t : REAL := 0.0;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : REAL;
    			END_VAR
    				a0 := 0;
    				a1 := 0.0;
    				a0.%X0 := t_is_some;
    				IF t_is_some THEN
    					a1 := t;
    				END_IF;
    				WARM_IN_RAW(a0 := a0, a1 := a1);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}
