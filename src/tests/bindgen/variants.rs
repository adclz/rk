// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Variants, and results with a value and an error: what no record of
//! outputs is laid out as. The import, `<NAME>_RAW`, gives the result in
//! words, and `<NAME>` reads each value out of them where the canonical ABI
//! puts it.

use std::pin::Pin;
use std::task::{Context, Poll};

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::component::{Linker, Val};

use super::{checked, hosted};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// A result with a value and an error.
#[rstest]
fn a_result_with_a_value_and_an_error_is_read_out_of_words(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface sockets {
            variant error-code {
                access-denied,
                not-supported,
                other(option<string>),
            }
            resource socket {
                hop-limit: func() -> result<u8, error-code>;
                buffer-size: func() -> result<u64, error-code>;
                set-hop-limit: func(value: u8) -> result<_, error-code>;
            }
            open: func() -> result<socket, error-code>;
        }

        world example {
            import sockets;
        }
    "#), @r"
    // wasi-example-sockets.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Sockets
    			USING Std.Convert;

    			TYPE ERROR_CODE : USINT (
    				ACCESS_DENIED := 0,
    				NOT_SUPPORTED := 1,
    				OTHER := 2
    			); END_TYPE

    			/* Lets go of `handle`, a handle of `socket` the host gave. */
    			{extern 'wasi:example/sockets@0.3.0' '[resource-drop]socket'}
    			FUNCTION SOCKET_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* `[method]socket.hop-limit` as the host gives it: its result in words of
    			   8 bytes, as the canonical ABI lays it out, which `SOCKET_HOP_LIMIT`
    			   reads. */
    			{extern 'wasi:example/sockets@0.3.0' '[method]socket.hop-limit'}
    			FUNCTION PRIVATE SOCKET_HOP_LIMIT_RAW
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION SOCKET_HOP_LIMIT
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : USINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				SOCKET_HOP_LIMIT_RAW(self := self, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value := BYTE_TO_USINT(w0.%B4);
    				END_IF;
    			END_FUNCTION

    			/* `[method]socket.buffer-size` as the host gives it: its result in words
    			   of 8 bytes, as the canonical ABI lays it out, which `SOCKET_BUFFER_SIZE`
    			   reads. */
    			{extern 'wasi:example/sockets@0.3.0' '[method]socket.buffer-size'}
    			FUNCTION PRIVATE SOCKET_BUFFER_SIZE_RAW
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION SOCKET_BUFFER_SIZE
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : ULINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				SOCKET_BUFFER_SIZE_RAW(self := self, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w1.%B0);
    				ELSE
    					value := LWORD_TO_ULINT(w1);
    				END_IF;
    			END_FUNCTION

    			/* `[method]socket.set-hop-limit` as the host gives it: its result in words
    			   of 8 bytes, as the canonical ABI lays it out, which
    			   `SOCKET_SET_HOP_LIMIT` reads. */
    			{extern 'wasi:example/sockets@0.3.0' '[method]socket.set-hop-limit'}
    			FUNCTION PRIVATE SOCKET_SET_HOP_LIMIT_RAW
    			VAR_INPUT
    				self : UDINT;
    				value : USINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION SOCKET_SET_HOP_LIMIT
    			VAR_INPUT
    				self : UDINT;
    				value : USINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				SOCKET_SET_HOP_LIMIT_RAW(self := self, value := value, w0 => w0, w1 => w1, w2 => w2);
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				END_IF;
    			END_FUNCTION

    			/* `open` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `OPEN` reads. */
    			{extern 'wasi:example/sockets@0.3.0' 'open'}
    			FUNCTION PRIVATE OPEN_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION OPEN
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				OPEN_RAW(w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value := DWORD_TO_UDINT(w0.%D1);
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `error-code` holds: a string, which an import cannot
         give back yet. */
    ");
}

/// A variant whose cases hold values.
#[rstest]
fn a_variant_is_its_case_and_what_each_case_holds(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface shapes {
            record point { x: s32, y: s32 }
            variant shape {
                nothing,
                circle(f32),
                square(u16),
                at(point),
                pair(tuple<u8, u64>),
                named(string),
            }
            pick: func() -> shape;
            find: func(id: u32) -> option<shape>;
            measure: func() -> result<shape, u32>;
        }

        world example {
            import shapes;
        }
    "#), @r"
    // wasi-example-shapes.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Shapes
    			USING Std.Convert;

    			TYPE SHAPE : USINT (
    				NOTHING := 0,
    				CIRCLE := 1,
    				SQUARE := 2,
    				AT_ := 3,
    				PAIR := 4,
    				NAMED := 5
    			); END_TYPE

    			/* `pick` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `PICK` reads. */
    			{extern 'wasi:example/shapes@0.3.0' 'pick'}
    			FUNCTION PRIVATE PICK_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION PICK
    			VAR_OUTPUT
    				value : SHAPE;
    				circle : REAL;
    				square : UINT;
    				at_x : DINT;
    				at_y : DINT;
    				pair_0 : USINT;
    				pair_1 : ULINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				PICK_RAW(w0 => w0, w1 => w1, w2 => w2);
    				circle := 0.0;
    				square := 0;
    				at_x := 0;
    				at_y := 0;
    				pair_0 := 0;
    				pair_1 := 0;
    				value := BYTE_TO_USINT(w0.%B0);
    				CASE value OF
    					SHAPE#CIRCLE:
    						circle := DWORD_TO_REAL(w1.%D0);
    					SHAPE#SQUARE:
    						square := WORD_TO_UINT(w1.%W0);
    					SHAPE#AT_:
    						at_x := DWORD_TO_DINT(w1.%D0);
    						at_y := DWORD_TO_DINT(w1.%D1);
    					SHAPE#PAIR:
    						pair_0 := BYTE_TO_USINT(w1.%B0);
    						pair_1 := LWORD_TO_ULINT(w2);
    				END_CASE;
    			END_FUNCTION

    			/* `find` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FIND` reads. */
    			{extern 'wasi:example/shapes@0.3.0' 'find'}
    			FUNCTION PRIVATE FIND_RAW
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FIND
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : SHAPE;
    				value_circle : REAL;
    				value_square : UINT;
    				value_at_x : DINT;
    				value_at_y : DINT;
    				value_pair_0 : USINT;
    				value_pair_1 : ULINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    				FIND_RAW(id := id, w0 => w0, w1 => w1, w2 => w2, w3 => w3);
    				value := 0;
    				value_circle := 0.0;
    				value_square := 0;
    				value_at_x := 0;
    				value_at_y := 0;
    				value_pair_0 := 0;
    				value_pair_1 := 0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value := BYTE_TO_USINT(w1.%B0);
    					CASE value OF
    						SHAPE#CIRCLE:
    							value_circle := DWORD_TO_REAL(w2.%D0);
    						SHAPE#SQUARE:
    							value_square := WORD_TO_UINT(w2.%W0);
    						SHAPE#AT_:
    							value_at_x := DWORD_TO_DINT(w2.%D0);
    							value_at_y := DWORD_TO_DINT(w2.%D1);
    						SHAPE#PAIR:
    							value_pair_0 := BYTE_TO_USINT(w2.%B0);
    							value_pair_1 := LWORD_TO_ULINT(w3);
    					END_CASE;
    				END_IF;
    			END_FUNCTION

    			/* `measure` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `MEASURE` reads. */
    			{extern 'wasi:example/shapes@0.3.0' 'measure'}
    			FUNCTION PRIVATE MEASURE_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MEASURE
    			VAR_OUTPUT
    				failed : BOOL;
    				value : SHAPE;
    				value_circle : REAL;
    				value_square : UINT;
    				value_at_x : DINT;
    				value_at_y : DINT;
    				value_pair_0 : USINT;
    				value_pair_1 : ULINT;
    				error : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    				MEASURE_RAW(w0 => w0, w1 => w1, w2 => w2, w3 => w3);
    				value := 0;
    				value_circle := 0.0;
    				value_square := 0;
    				value_at_x := 0;
    				value_at_y := 0;
    				value_pair_0 := 0;
    				value_pair_1 := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := DWORD_TO_UDINT(w1.%D0);
    				ELSE
    					value := BYTE_TO_USINT(w1.%B0);
    					CASE value OF
    						SHAPE#CIRCLE:
    							value_circle := DWORD_TO_REAL(w2.%D0);
    						SHAPE#SQUARE:
    							value_square := WORD_TO_UINT(w2.%W0);
    						SHAPE#AT_:
    							value_at_x := DWORD_TO_DINT(w2.%D0);
    							value_at_y := DWORD_TO_DINT(w2.%D1);
    						SHAPE#PAIR:
    							value_pair_0 := BYTE_TO_USINT(w2.%B0);
    							value_pair_1 := LWORD_TO_ULINT(w3);
    					END_CASE;
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `named` of `shape` holds: a string, which an import cannot give
         back yet. */
    ");
}

/// A variant of another interface is named from the root, its cases too,
/// and an async function's result is read by the block that calls its
/// import, in the call that sees it finish.
#[rstest]
fn a_variant_of_another_interface_and_an_async_function(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface types {
            variant error-code { busy, limit(u32), other(option<string>) }
            variant address { v4(tuple<u8, u8, u8, u8>), v6(tuple<u16, u16>) }
        }

        interface lookup {
            use types.{error-code, address};
            resolve: func(id: u32) -> result<address, error-code>;
            first: async func(name: string) -> option<address>;
        }

        world example {
            import lookup;
        }
    "#), @r"
    // wasi-example-types.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Types
    			TYPE ERROR_CODE : USINT (
    				BUSY := 0,
    				LIMIT := 1,
    				OTHER := 2
    			); END_TYPE

    			TYPE ADDRESS : USINT (
    				V4 := 0,
    				V6 := 1
    			); END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `error-code` holds: a string, which an import cannot
         give back yet. */

    // wasi-example-lookup.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Lookup
    			USING Std.Convert;

    			/* `resolve` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `RESOLVE` reads. */
    			{extern 'wasi:example/lookup@0.3.0' 'resolve'}
    			FUNCTION PRIVATE RESOLVE_RAW
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION RESOLVE
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : Wasi.Example.Types.ADDRESS;
    				value_v4_0 : USINT;
    				value_v4_1 : USINT;
    				value_v4_2 : USINT;
    				value_v4_3 : USINT;
    				value_v6_0 : UINT;
    				value_v6_1 : UINT;
    				error : Wasi.Example.Types.ERROR_CODE;
    				error_limit : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				RESOLVE_RAW(id := id, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				value_v4_0 := 0;
    				value_v4_1 := 0;
    				value_v4_2 := 0;
    				value_v4_3 := 0;
    				value_v6_0 := 0;
    				value_v6_1 := 0;
    				error := 0;
    				error_limit := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    					CASE error OF
    						Wasi.Example.Types.ERROR_CODE#LIMIT:
    							error_limit := DWORD_TO_UDINT(w1.%D0);
    					END_CASE;
    				ELSE
    					value := BYTE_TO_USINT(w0.%B4);
    					CASE value OF
    						Wasi.Example.Types.ADDRESS#V4:
    							value_v4_0 := BYTE_TO_USINT(w0.%B6);
    							value_v4_1 := BYTE_TO_USINT(w0.%B7);
    							value_v4_2 := BYTE_TO_USINT(w1.%B0);
    							value_v4_3 := BYTE_TO_USINT(w1.%B1);
    						Wasi.Example.Types.ADDRESS#V6:
    							value_v6_0 := WORD_TO_UINT(w0.%W3);
    							value_v6_1 := WORD_TO_UINT(w1.%W0);
    					END_CASE;
    				END_IF;
    			END_FUNCTION

    			/* `first` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FIRST` reads. */
    			{extern async 'wasi:example/lookup@0.3.0' 'first'}
    			FUNCTION_BLOCK FIRST_RAW
    			VAR_INPUT
    				name : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				w0 : LWORD;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK FIRST
    			VAR_INPUT
    				name : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				is_some : BOOL;
    				value : Wasi.Example.Types.ADDRESS;
    				value_v4_0 : USINT;
    				value_v4_1 : USINT;
    				value_v4_2 : USINT;
    				value_v4_3 : USINT;
    				value_v6_0 : UINT;
    				value_v6_1 : UINT;
    			END_VAR
    			VAR
    				call : FIRST_RAW;
    			END_VAR
    				call(name := name);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value := 0;
    					value_v4_0 := 0;
    					value_v4_1 := 0;
    					value_v4_2 := 0;
    					value_v4_3 := 0;
    					value_v6_0 := 0;
    					value_v6_1 := 0;
    					is_some := call.w0.%X0;
    					IF is_some THEN
    						value := BYTE_TO_USINT(call.w0.%B2);
    						CASE value OF
    							Wasi.Example.Types.ADDRESS#V4:
    								value_v4_0 := BYTE_TO_USINT(call.w0.%B4);
    								value_v4_1 := BYTE_TO_USINT(call.w0.%B5);
    								value_v4_2 := BYTE_TO_USINT(call.w0.%B6);
    								value_v4_3 := BYTE_TO_USINT(call.w0.%B7);
    							Wasi.Example.Types.ADDRESS#V6:
    								value_v6_0 := WORD_TO_UINT(call.w0.%W2);
    								value_v6_1 := WORD_TO_UINT(call.w0.%W3);
    						END_CASE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A variant none of whose cases holds a value is a list of names, as an
/// enum is: a scalar, passed and returned as it is.
#[rstest]
fn a_variant_that_holds_nothing_is_its_case(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface plain {
            variant state { idle, running, stopped }
            current: func() -> state;
            set: func(to: state);
            history: func() -> tuple<state, state>;
        }

        world example {
            import plain;
        }
    "#), @r"
    // wasi-example-plain.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Plain
    			TYPE STATE : USINT (
    				IDLE := 0,
    				RUNNING := 1,
    				STOPPED := 2
    			); END_TYPE

    			{extern 'wasi:example/plain@0.3.0' 'current'}
    			FUNCTION CURRENT : STATE
    			END_FUNCTION

    			{extern 'wasi:example/plain@0.3.0' 'set'}
    			FUNCTION SET
    			VAR_INPUT
    				to_ : STATE;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/plain@0.3.0' 'history'}
    			FUNCTION HISTORY
    			VAR_OUTPUT
    				item0 : STATE;
    				item1 : STATE;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What is not read: a case's value that holds a list, which leaves the
/// case its name; two values that would share a name; a compound the
/// canonical ABI returns as one value, not in memory.
#[rstest]
fn what_is_not_read_out_of_words(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface unread {
            record wrapped { inner: tuple<u32> }
            variant found { nothing, bytes(list<u8>), size(u64) }
            variant choice { value(u32), none }
            find: func() -> found;
            choose: func() -> choice;
            wrap: func() -> wrapped;
        }

        world example {
            import unread;
        }
    "#), @r"
    // wasi-example-unread.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Unread
    			USING Std.Convert;

    			TYPE FOUND : USINT (
    				NOTHING := 0,
    				BYTES := 1,
    				SIZE := 2
    			); END_TYPE

    			TYPE CHOICE : USINT (
    				VALUE := 0,
    				NONE := 1
    			); END_TYPE

    			/* `find` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FIND` reads. */
    			{extern 'wasi:example/unread@0.3.0' 'find'}
    			FUNCTION PRIVATE FIND_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FIND
    			VAR_OUTPUT
    				value : FOUND;
    				size : ULINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				FIND_RAW(w0 => w0, w1 => w1);
    				size := 0;
    				value := BYTE_TO_USINT(w0.%B0);
    				CASE value OF
    					FOUND#SIZE:
    						size := LWORD_TO_ULINT(w1);
    				END_CASE;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `bytes` of `found` holds: a list, which an import cannot give
         back yet.
       - `choose`: two of its variables would be `value`, which ST takes for
         one.
       - `wrap`: its result holds a tuple, which an import cannot give back
         yet. */
    ");
}

/// A variant of more than 256 cases holds its case in two bytes: a UINT,
/// read as one, and what a case holds comes after it.
#[rstest]
fn a_variant_of_more_than_256_cases_has_its_case_in_a_uint(mut with_db: RootDatabase) {
    let cases: String = (0..257).map(|at| format!("c{at}, ")).collect();
    let wit = format!(
        r#"
        package wasi:example@0.3.0;

        interface many {{
            variant big {{ {cases}last(u32) }}
            pick: func() -> big;
        }}

        world example {{
            import many;
        }}
    "#
    );
    let text = checked(&mut with_db, &wit);
    for line in [
        "TYPE BIG : UINT (",
        "value := WORD_TO_UINT(w0.%W0);",
        "BIG#LAST:",
        "last := DWORD_TO_UDINT(w0.%D1);",
    ] {
        assert!(text.contains(line), "`{line}` in:\n{text}");
    }
}

/// Every kind of value a case holds, an option and a result of a variant,
/// a record that holds an option and a tuple, and an async function.
const SHAPES: &str = r#"
    package wasi:example@0.3.0;

    interface shapes {
        record point { x: s32, y: s32 }
        enum color { red, green, blue }
        flags marks { bold, dashed, filled }
        variant shape {
            nothing,
            circle(f32),
            square(u16),
            at(point),
            pair(tuple<s8, u64>),
            tinted(color),
            marked(marks),
            wide(f64),
            signed(s16),
            long(s64),
            named(string),
        }
        variant error-code { busy, gone, other(option<string>) }
        record outer { a: u8, at: option<point>, tail: tuple<bool, u32> }
        pick: func(which: u32) -> shape;
        find: func(which: u32) -> option<shape>;
        measure: func(which: u32) -> result<shape, error-code>;
        count: func(which: u32) -> result<u64, error-code>;
        level: func(which: u32) -> result<u8, error-code>;
        inner: func(which: u32) -> outer;
        wait: async func(which: u32) -> result<point, error-code>;
    }

    world example {
        import shapes;
    }
"#;

/// A point, as the host gives one.
fn point(x: i32, y: i32) -> Val {
    Val::Record(vec![
        ("x".to_string(), Val::S32(x)),
        ("y".to_string(), Val::S32(y)),
    ])
}

/// A case that holds `value`, or holds nothing.
fn case(name: &str, value: Option<Val>) -> Val {
    Val::Variant(name.to_string(), value.map(Box::new))
}

/// The shape the host gives for `which`: one of each case.
fn shape(which: u32) -> Val {
    match which {
        1 => case("circle", Some(Val::Float32(1.5))),
        2 => case("square", Some(Val::U16(65535))),
        3 => case("at", Some(point(-1, 2))),
        4 => case(
            "pair",
            Some(Val::Tuple(vec![
                Val::S8(-5),
                Val::U64(0x0102_0304_0506_0708),
            ])),
        ),
        5 => case("tinted", Some(Val::Enum("blue".to_string()))),
        6 => case(
            "marked",
            Some(Val::Flags(vec!["bold".to_string(), "filled".to_string()])),
        ),
        7 => case("wide", Some(Val::Float64(-2.25))),
        8 => case("signed", Some(Val::S16(-300))),
        9 => case("long", Some(Val::S64(-(1 << 40)))),
        10 => case("named", Some(Val::String("text".to_string()))),
        _ => case("nothing", None),
    }
}

/// The error the host gives for `which`: `gone`, or `other` with its text.
fn error(which: u32) -> Val {
    match which {
        2 => case(
            "other",
            Some(Val::Option(Some(Box::new(Val::String("why".to_string()))))),
        ),
        _ => case("gone", None),
    }
}

fn ok(value: Val) -> Val {
    Val::Result(Ok(Some(Box::new(value))))
}

fn failed(which: u32) -> Val {
    Val::Result(Err(Some(Box::new(error(which)))))
}

/// A future that is not ready the first time it is asked.
struct Later(bool);

impl Future for Later {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// The host's side of [`SHAPES`]: what each function returns for `which`.
fn shapes_host(linker: &mut Linker<Host>) {
    let which = |params: &[Val]| match params {
        [Val::U32(which)] => *which,
        other => panic!("`which`, a u32: {other:?}"),
    };
    let mut shapes = linker
        .instance("wasi:example/shapes@0.3.0")
        .expect("the interface");
    let mut returns = |name: &str, value: fn(u32) -> Val| {
        shapes
            .func_new(name, move |_, _, params, results| {
                results[0] = value(which(params));
                Ok(())
            })
            .expect("a function of the host");
    };
    returns("pick", shape);
    returns("find", |which| match which {
        1 => Val::Option(Some(Box::new(case("circle", Some(Val::Float32(2.5)))))),
        2 => Val::Option(Some(Box::new(case("at", Some(point(7, -8)))))),
        _ => Val::Option(None),
    });
    returns("measure", |which| match which {
        0 => ok(case("square", Some(Val::U16(9)))),
        which => failed(which),
    });
    returns("count", |which| match which {
        0 => ok(Val::U64(u64::MAX - 1)),
        which => failed(which),
    });
    returns("level", |which| match which {
        0 => ok(Val::U8(200)),
        which => failed(which),
    });
    returns("inner", |which| {
        let (a, at, tail) = match which {
            0 => (9, Some(Box::new(point(3, -4))), (true, 77)),
            _ => (1, None, (false, 4_000_000_000)),
        };
        Val::Record(vec![
            ("a".to_string(), Val::U8(a)),
            ("at".to_string(), Val::Option(at)),
            (
                "tail".to_string(),
                Val::Tuple(vec![Val::Bool(tail.0), Val::U32(tail.1)]),
            ),
        ])
    });
    shapes
        .func_new_concurrent("wait", move |_, _, params, results| {
            Box::pin(async move {
                Later(false).await;
                results[0] = match which(params) {
                    0 => ok(point(5, -6)),
                    which => failed(which),
                };
                Ok(())
            })
        })
        .expect("an async function of the host");
}

/// What the generated code reads back from the host of [`SHAPES`]: 1 when
/// every value is what the host returned and every value of a case that
/// did not come is zero, and otherwise the number of the check that
/// failed, negative.
const READ_BACK: &str = r#"
    /* Whether `which` comes back as the case `shape`, with `nonzero` of the
       values its cases hold not zero. */
    FUNCTION picked : BOOL
    VAR_INPUT which : UDINT; shape : USINT; nonzero : DINT; END_VAR
    VAR_OUTPUT
        circle : REAL; square : UINT; x : DINT; y : DINT; first : SINT; second : ULINT;
        tinted : Wasi.Example.Shapes.COLOR; marked : Wasi.Example.Shapes.MARKS;
        wide : LREAL; signed : INT; long : LINT;
    END_VAR
    VAR value : Wasi.Example.Shapes.SHAPE; count : DINT; END_VAR
        Wasi.Example.Shapes.PICK(which := which, value => value, circle => circle,
            square => square, at_x => x, at_y => y, pair_0 => first, pair_1 => second,
            tinted => tinted, marked => marked, wide => wide, signed => signed, long => long);
        count := 0;
        IF circle <> 0.0 THEN count := count + 1; END_IF;
        IF square <> 0 THEN count := count + 1; END_IF;
        IF x <> 0 THEN count := count + 1; END_IF;
        IF y <> 0 THEN count := count + 1; END_IF;
        IF first <> 0 THEN count := count + 1; END_IF;
        IF second <> 0 THEN count := count + 1; END_IF;
        IF tinted <> 0 THEN count := count + 1; END_IF;
        IF marked <> 0 THEN count := count + 1; END_IF;
        IF wide <> 0.0 THEN count := count + 1; END_IF;
        IF signed <> 0 THEN count := count + 1; END_IF;
        IF long <> 0 THEN count := count + 1; END_IF;
        picked := value = shape AND count = nonzero;
    END_FUNCTION

    FUNCTION check : DINT
    VAR
        circle : REAL; square : UINT; x : DINT; y : DINT; first : SINT; second : ULINT;
        tinted : Wasi.Example.Shapes.COLOR; marked : Wasi.Example.Shapes.MARKS;
        wide : LREAL; signed : INT; long : LINT;
        ok, some, failed, flag : BOOL;
        shape : Wasi.Example.Shapes.SHAPE;
        error : Wasi.Example.Shapes.ERROR_CODE;
        count : ULINT; level : USINT; a : USINT; tail : UDINT;
    END_VAR
        check := 0;
        // `at`, then `circle`: what `at` held is gone.
        ok := picked(which := 3, shape := 3, nonzero := 2, x => x, y => y);
        IF NOT ok OR x <> -1 OR y <> 2 THEN check := -1; RETURN; END_IF;
        ok := picked(which := 1, shape := 1, nonzero := 1, circle => circle);
        IF NOT ok OR circle <> 1.5 THEN check := -2; RETURN; END_IF;
        ok := picked(which := 4, shape := 4, nonzero := 2, first => first, second => second);
        IF NOT ok OR first <> -5 OR second <> ULINT#16#0102030405060708 THEN check := -3; RETURN; END_IF;
        ok := picked(which := 2, shape := 2, nonzero := 1, square => square);
        IF NOT ok OR square <> 65535 THEN check := -4; RETURN; END_IF;
        ok := picked(which := 0, shape := 0, nonzero := 0);
        IF NOT ok THEN check := -5; RETURN; END_IF;
        ok := picked(which := 5, shape := 5, nonzero := 1, tinted => tinted);
        IF NOT ok OR tinted <> 2 THEN check := -6; RETURN; END_IF;
        ok := picked(which := 6, shape := 6, nonzero := 1, marked => marked);
        IF NOT ok OR marked <> 16#05 THEN check := -7; RETURN; END_IF;
        ok := picked(which := 7, shape := 7, nonzero := 1, wide => wide);
        IF NOT ok OR wide <> -2.25 THEN check := -8; RETURN; END_IF;
        ok := picked(which := 8, shape := 8, nonzero := 1, signed => signed);
        IF NOT ok OR signed <> -300 THEN check := -9; RETURN; END_IF;
        ok := picked(which := 9, shape := 9, nonzero := 1, long => long);
        IF NOT ok OR long <> LINT#-1099511627776 THEN check := -10; RETURN; END_IF;
        // A case whose value is a string comes back as its name alone.
        ok := picked(which := 10, shape := 10, nonzero := 0);
        IF NOT ok THEN check := -11; RETURN; END_IF;

        Wasi.Example.Shapes.FIND(which := 1, is_some => some, value => shape,
            value_circle => circle);
        IF NOT some OR shape <> 1 OR circle <> 2.5 THEN check := -12; RETURN; END_IF;
        Wasi.Example.Shapes.FIND(which := 2, is_some => some, value => shape,
            value_circle => circle, value_at_x => x, value_at_y => y);
        IF NOT some OR shape <> 3 OR x <> 7 OR y <> -8 OR circle <> 0.0 THEN
            check := -13; RETURN;
        END_IF;
        Wasi.Example.Shapes.FIND(which := 0, is_some => some, value => shape,
            value_at_x => x, value_at_y => y);
        IF some OR shape <> 0 OR x <> 0 OR y <> 0 THEN check := -14; RETURN; END_IF;

        Wasi.Example.Shapes.MEASURE(which := 0, failed => failed, value => shape,
            value_square => square, error => error);
        IF failed OR shape <> 2 OR square <> 9 OR error <> 0 THEN check := -15; RETURN; END_IF;
        Wasi.Example.Shapes.MEASURE(which := 1, failed => failed, value => shape,
            value_square => square, error => error);
        IF NOT failed OR error <> 1 OR shape <> 0 OR square <> 0 THEN
            check := -16; RETURN;
        END_IF;
        Wasi.Example.Shapes.MEASURE(which := 2, failed => failed, error => error);
        IF NOT failed OR error <> 2 THEN check := -17; RETURN; END_IF;

        Wasi.Example.Shapes.COUNT(which := 0, failed => failed, value => count, error => error);
        IF failed OR count <> ULINT#18446744073709551614 OR error <> 0 THEN
            check := -18; RETURN;
        END_IF;
        Wasi.Example.Shapes.COUNT(which := 1, failed => failed, value => count, error => error);
        IF NOT failed OR error <> 1 OR count <> 0 THEN check := -19; RETURN; END_IF;

        Wasi.Example.Shapes.LEVEL(which := 0, failed => failed, value => level, error => error);
        IF failed OR level <> 200 OR error <> 0 THEN check := -20; RETURN; END_IF;
        Wasi.Example.Shapes.LEVEL(which := 1, failed => failed, value => level, error => error);
        IF NOT failed OR error <> 1 OR level <> 0 THEN check := -21; RETURN; END_IF;

        Wasi.Example.Shapes.INNER(which := 0, a => a, at_is_some => some, at_x => x, at_y => y,
            tail_0 => flag, tail_1 => tail);
        IF a <> 9 OR NOT some OR x <> 3 OR y <> -4 OR NOT flag OR tail <> 77 THEN
            check := -22; RETURN;
        END_IF;
        Wasi.Example.Shapes.INNER(which := 1, a => a, at_is_some => some, at_x => x, at_y => y,
            tail_0 => flag, tail_1 => tail);
        IF a <> 1 OR some OR x <> 0 OR y <> 0 OR flag OR tail <> UDINT#4000000000 THEN
            check := -23; RETURN;
        END_IF;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR
        checked : BOOL;
        step : INT;
        wait : Wasi.Example.Shapes.WAIT;
    END_VAR
        IF NOT checked THEN
            checked := TRUE;
            output := check();
            IF output <> 0 THEN step := 9; END_IF;
        END_IF;
        CASE step OF
        0:
            wait(which := 0);
            IF wait.DONE THEN
                IF wait.failed OR wait.value_x <> 5 OR wait.value_y <> -6 OR wait.error <> 0 THEN
                    output := -30;
                    step := 9;
                ELSE
                    step := 1;
                END_IF;
            END_IF;
        1:
            // An error after a value: the value is gone.
            wait(which := 1);
            IF wait.DONE THEN
                IF NOT wait.failed OR wait.error <> 1 OR wait.value_x <> 0 OR wait.value_y <> 0 THEN
                    output := -31;
                ELSE
                    output := 1;
                END_IF;
                step := 9;
            END_IF;
        END_CASE;
    END_PROGRAM
"#;

/// What wasmtime lays out for each case of a variant, an option and a
/// result of one, a result with a value and an error, a record holding an
/// option and a tuple, and an async function's result, read back by the
/// generated code: every value the host returned, and zero in every value
/// of a case that did not come.
#[rstest]
fn what_a_host_returns_is_read_back(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, SHAPES, READ_BACK, shapes_host);
    let mut output = 0;
    for _ in 0..20 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}
