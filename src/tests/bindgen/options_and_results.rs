// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Options and results that come back: an option is its flag `is_some`
//! then its `value`, a result its flag `failed` then its ok `value` or its
//! `error`, laid out as the canonical ABI lays the variant out. A result
//! with both, whose value and error share their place, and an option of a
//! record come back in words, read out of them. Either passed in is left
//! out.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
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

    			/* Lets go of `handle`, a handle of `entry` the host gave. */
    			{extern 'wasi:example/finder@0.3.0' '[resource-drop]entry'}
    			FUNCTION ENTRY_DROP
    			VAR_INPUT
    				handle : UDINT;
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
    				value : UDINT;
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

    			/* Lets go of `handle`, a handle of `handle` the host gave. */
    			{extern 'wasi:example/outcomes@0.3.0' '[resource-drop]handle'}
    			FUNCTION HANDLE_DROP
    			VAR_INPUT
    				handle : UDINT;
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
    				value : UDINT;
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
    				value_x : LREAL;
    				value_y : LREAL;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				WHERE_IS_RAW(w0 => w0, w1 => w1, w2 => w2);
    				value_x := 0.0;
    				value_y := 0.0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value_x := LWORD_TO_LREAL(w1);
    					value_y := LWORD_TO_LREAL(w2);
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
    				value_x : LREAL;
    				value_y : LREAL;
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
    				value_x := 0.0;
    				value_y := 0.0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w1.%B0);
    				ELSE
    					value_is_some := w1.%X0;
    					IF value_is_some THEN
    						value_x := LWORD_TO_LREAL(w2);
    						value_y := LWORD_TO_LREAL(w3);
    					END_IF;
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What cannot come back or pass yet: a result of a string, an option or
/// a result passed in.
#[rstest]
fn an_option_or_a_result_that_cannot_pass_is_left_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface leftovers {
            enum error-code { busy }
            name: func() -> result<string>;
            maybe-in: func(x: option<u32>);
            result-in: func(x: result<_, error-code>);
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
    			TYPE ERROR_CODE : USINT (
    				BUSY := 0
    			); END_TYPE

    			{extern 'wasi:example/leftovers@0.3.0' 'kept'}
    			FUNCTION KEPT : USINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `name`: its result holds a string, which an import cannot give back
         yet.
       - `maybe-in`: its parameter `x` holds an option, which an import cannot
         take yet.
       - `result-in`: its parameter `x` holds a result, which an import cannot
         take yet. */
    ");
}
