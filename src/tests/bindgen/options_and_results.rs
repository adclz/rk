// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Options and results that come back: an option is its flag `is_some`
//! then its `value`, a result its flag `failed` then its ok `value` or its
//! `error`, laid out as the canonical ABI lays the variant out. A result
//! with both, an option of a compound, and either passed in, are left out.

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

/// What two outputs cannot hold: a result with a value and an error,
/// which share their place in memory; an option or a result of a record or
/// a string; an option or a result passed in.
#[rstest]
fn what_two_outputs_cannot_hold_is_left_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface leftovers {
            enum error-code { busy }
            record point { x: f64, y: f64 }
            both: func() -> result<u32, error-code>;
            where-is: func() -> option<point>;
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
       - `both`: its result holds a result with a value and an error, which an
         import cannot give back yet.
       - `where-is`: its result holds a record, which an import cannot give
         back yet.
       - `name`: its result holds a string, which an import cannot give back
         yet.
       - `maybe-in`: its parameter `x` holds an option, which an import cannot
         take yet.
       - `result-in`: its parameter `x` holds a result, which an import cannot
         take yet. */
    ");
}
