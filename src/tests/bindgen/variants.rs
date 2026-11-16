// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Variants, and results with a value and an error: what no record of
//! outputs is laid out as. The import, `<NAME>_RAW`, gives the result in
//! words, and `<NAME>` reads each value out of them where the canonical ABI
//! puts it. A variant passed in is its case and what each case holds, which
//! `<NAME>` packs into the words `<NAME>_RAW` takes: the values the cases
//! share, or the bytes of the tuple an async call's parameters are in past
//! 4 values.

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

    			/* The cases of `error-code`. */
    			TYPE ERROR_CODE_KIND : USINT (
    				ACCESS_DENIED := 0,
    				NOT_SUPPORTED := 1,
    				OTHER := 2
    			); END_TYPE

    			TYPE ERROR_CODE : STRUCT
    				kind : ERROR_CODE_KIND;
    				other_is_some : BOOL;
    				other : STRING[256];
    			END_STRUCT END_TYPE

    			/* Lets go of `handle`, a handle of `socket` the host gave. */
    			{extern 'wasi:example/sockets@0.3.0' '[resource-drop]socket'}
    			FUNCTION SOCKET_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* `[method]socket.hop-limit` as the host gives it: its result in words of
    			   8 bytes, as the canonical ABI lays it out, which `SOCKET_HOP_LIMIT`
    			   reads, and its text in `other`, where the host writes it, cut to fit. */
    			{extern 'wasi:example/sockets@0.3.0' '[method]socket.hop-limit'}
    			FUNCTION PRIVATE SOCKET_HOP_LIMIT_RAW
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				other : STRING[256];
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
    				SOCKET_HOP_LIMIT_RAW(self := self, other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error.kind := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    					CASE error.kind OF
    						ERROR_CODE_KIND#OTHER:
    							error.other_is_some := w1.%X0;
    					END_CASE;
    				ELSE
    					value := BYTE_TO_USINT(w0.%B4);
    				END_IF;
    			END_FUNCTION

    			/* `[method]socket.buffer-size` as the host gives it: its result in words
    			   of 8 bytes, as the canonical ABI lays it out, which `SOCKET_BUFFER_SIZE`
    			   reads, and its text in `other`, where the host writes it, cut to fit. */
    			{extern 'wasi:example/sockets@0.3.0' '[method]socket.buffer-size'}
    			FUNCTION PRIVATE SOCKET_BUFFER_SIZE_RAW
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				other : STRING[256];
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
    				SOCKET_BUFFER_SIZE_RAW(self := self, other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error.kind := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w1.%B0);
    					CASE error.kind OF
    						ERROR_CODE_KIND#OTHER:
    							error.other_is_some := w1.%X32;
    					END_CASE;
    				ELSE
    					value := LWORD_TO_ULINT(w1);
    				END_IF;
    			END_FUNCTION

    			/* `[method]socket.set-hop-limit` as the host gives it: its result in words
    			   of 8 bytes, as the canonical ABI lays it out, which
    			   `SOCKET_SET_HOP_LIMIT` reads, and its text in `other`, where the host
    			   writes it, cut to fit. */
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
    				other : STRING[256];
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
    				SOCKET_SET_HOP_LIMIT_RAW(self := self, value := value, other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				error.kind := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    					CASE error.kind OF
    						ERROR_CODE_KIND#OTHER:
    							error.other_is_some := w1.%X0;
    					END_CASE;
    				END_IF;
    			END_FUNCTION

    			/* `open` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `OPEN` reads, and its text in `other`,
    			   where the host writes it, cut to fit. */
    			{extern 'wasi:example/sockets@0.3.0' 'open'}
    			FUNCTION PRIVATE OPEN_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				other : STRING[256];
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
    				OPEN_RAW(other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error.kind := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    					CASE error.kind OF
    						ERROR_CODE_KIND#OTHER:
    							error.other_is_some := w1.%X0;
    					END_CASE;
    				ELSE
    					value := DWORD_TO_UDINT(w0.%D1);
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `error-code` holds: a string, which an async call
         cannot give back yet. */
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

    			TYPE POINT : STRUCT
    				x : DINT;
    				y : DINT;
    			END_STRUCT END_TYPE

    			/* The cases of `shape`. */
    			TYPE SHAPE_KIND : USINT (
    				NOTHING := 0,
    				CIRCLE := 1,
    				SQUARE := 2,
    				AT_ := 3,
    				PAIR := 4,
    				NAMED := 5
    			); END_TYPE

    			TYPE SHAPE : STRUCT
    				kind : SHAPE_KIND;
    				circle : REAL;
    				square : UINT;
    				at_ : POINT;
    				pair_0 : USINT;
    				pair_1 : ULINT;
    				named : STRING[256];
    			END_STRUCT END_TYPE

    			/* `pick` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `PICK` reads, and its text in `named`,
    			   where the host writes it, cut to fit. */
    			{extern 'wasi:example/shapes@0.3.0' 'pick'}
    			FUNCTION PRIVATE PICK_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				named : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION PICK : SHAPE
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				PICK_RAW(named => PICK.named, w0 => w0, w1 => w1, w2 => w2);
    				PICK.circle := 0.0;
    				PICK.square := 0;
    				PICK.at_.x := 0;
    				PICK.at_.y := 0;
    				PICK.pair_0 := 0;
    				PICK.pair_1 := 0;
    				PICK.kind := BYTE_TO_USINT(w0.%B0);
    				CASE PICK.kind OF
    					SHAPE_KIND#CIRCLE:
    						PICK.circle := DWORD_TO_REAL(w1.%D0);
    					SHAPE_KIND#SQUARE:
    						PICK.square := WORD_TO_UINT(w1.%W0);
    					SHAPE_KIND#AT_:
    						PICK.at_.x := DWORD_TO_DINT(w1.%D0);
    						PICK.at_.y := DWORD_TO_DINT(w1.%D1);
    					SHAPE_KIND#PAIR:
    						PICK.pair_0 := BYTE_TO_USINT(w1.%B0);
    						PICK.pair_1 := LWORD_TO_ULINT(w2);
    				END_CASE;
    			END_FUNCTION

    			/* `find` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FIND` reads, and its text in `named`,
    			   where the host writes it, cut to fit. */
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
    				named : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION FIND
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : SHAPE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    				FIND_RAW(id := id, named => value.named, w0 => w0, w1 => w1, w2 => w2, w3 => w3);
    				value.kind := 0;
    				value.circle := 0.0;
    				value.square := 0;
    				value.at_.x := 0;
    				value.at_.y := 0;
    				value.pair_0 := 0;
    				value.pair_1 := 0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value.kind := BYTE_TO_USINT(w1.%B0);
    					CASE value.kind OF
    						SHAPE_KIND#CIRCLE:
    							value.circle := DWORD_TO_REAL(w2.%D0);
    						SHAPE_KIND#SQUARE:
    							value.square := WORD_TO_UINT(w2.%W0);
    						SHAPE_KIND#AT_:
    							value.at_.x := DWORD_TO_DINT(w2.%D0);
    							value.at_.y := DWORD_TO_DINT(w2.%D1);
    						SHAPE_KIND#PAIR:
    							value.pair_0 := BYTE_TO_USINT(w2.%B0);
    							value.pair_1 := LWORD_TO_ULINT(w3);
    					END_CASE;
    				END_IF;
    			END_FUNCTION

    			/* `measure` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `MEASURE` reads, and its text in
    			   `named`, where the host writes it, cut to fit. */
    			{extern 'wasi:example/shapes@0.3.0' 'measure'}
    			FUNCTION PRIVATE MEASURE_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    				named : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION MEASURE
    			VAR_OUTPUT
    				failed : BOOL;
    				value : SHAPE;
    				error : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    			END_VAR
    				MEASURE_RAW(named => value.named, w0 => w0, w1 => w1, w2 => w2, w3 => w3);
    				value.kind := 0;
    				value.circle := 0.0;
    				value.square := 0;
    				value.at_.x := 0;
    				value.at_.y := 0;
    				value.pair_0 := 0;
    				value.pair_1 := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := DWORD_TO_UDINT(w1.%D0);
    				ELSE
    					value.kind := BYTE_TO_USINT(w1.%B0);
    					CASE value.kind OF
    						SHAPE_KIND#CIRCLE:
    							value.circle := DWORD_TO_REAL(w2.%D0);
    						SHAPE_KIND#SQUARE:
    							value.square := WORD_TO_UINT(w2.%W0);
    						SHAPE_KIND#AT_:
    							value.at_.x := DWORD_TO_DINT(w2.%D0);
    							value.at_.y := DWORD_TO_DINT(w2.%D1);
    						SHAPE_KIND#PAIR:
    							value.pair_0 := BYTE_TO_USINT(w2.%B0);
    							value.pair_1 := LWORD_TO_ULINT(w3);
    					END_CASE;
    				END_IF;
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `named` of `shape` holds: a string, which an async call cannot
         give back yet. */
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
    			/* The cases of `error-code`. */
    			TYPE ERROR_CODE_KIND : USINT (
    				BUSY := 0,
    				LIMIT := 1,
    				OTHER := 2
    			); END_TYPE

    			TYPE ERROR_CODE : STRUCT
    				kind : ERROR_CODE_KIND;
    				limit : UDINT;
    				other_is_some : BOOL;
    				other : STRING[256];
    			END_STRUCT END_TYPE

    			/* The cases of `address`. */
    			TYPE ADDRESS_KIND : USINT (
    				V4 := 0,
    				V6 := 1
    			); END_TYPE

    			TYPE ADDRESS : STRUCT
    				kind : ADDRESS_KIND;
    				v4_0 : USINT;
    				v4_1 : USINT;
    				v4_2 : USINT;
    				v4_3 : USINT;
    				v6_0 : UINT;
    				v6_1 : UINT;
    			END_STRUCT END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `error-code` holds: a string, which an async call
         cannot give back yet. */

    // wasi-example-lookup.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Lookup
    			USING Std.Convert;

    			/* `resolve` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `RESOLVE` reads, and its text in
    			   `other`, where the host writes it, cut to fit. */
    			{extern 'wasi:example/lookup@0.3.0' 'resolve'}
    			FUNCTION PRIVATE RESOLVE_RAW
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				other : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION RESOLVE
    			VAR_INPUT
    				id : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : Wasi.Example.Types.ADDRESS;
    				error : Wasi.Example.Types.ERROR_CODE;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				RESOLVE_RAW(id := id, other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				value.kind := 0;
    				value.v4_0 := 0;
    				value.v4_1 := 0;
    				value.v4_2 := 0;
    				value.v4_3 := 0;
    				value.v6_0 := 0;
    				value.v6_1 := 0;
    				error.kind := 0;
    				error.limit := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    					CASE error.kind OF
    						Wasi.Example.Types.ERROR_CODE_KIND#LIMIT:
    							error.limit := DWORD_TO_UDINT(w1.%D0);
    						Wasi.Example.Types.ERROR_CODE_KIND#OTHER:
    							error.other_is_some := w1.%X0;
    					END_CASE;
    				ELSE
    					value.kind := BYTE_TO_USINT(w0.%B4);
    					CASE value.kind OF
    						Wasi.Example.Types.ADDRESS_KIND#V4:
    							value.v4_0 := BYTE_TO_USINT(w0.%B6);
    							value.v4_1 := BYTE_TO_USINT(w0.%B7);
    							value.v4_2 := BYTE_TO_USINT(w1.%B0);
    							value.v4_3 := BYTE_TO_USINT(w1.%B1);
    						Wasi.Example.Types.ADDRESS_KIND#V6:
    							value.v6_0 := WORD_TO_UINT(w0.%W3);
    							value.v6_1 := WORD_TO_UINT(w1.%W0);
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
    			END_VAR
    			VAR
    				call : FIRST_RAW;
    			END_VAR
    				call(name := name);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.kind := 0;
    					value.v4_0 := 0;
    					value.v4_1 := 0;
    					value.v4_2 := 0;
    					value.v4_3 := 0;
    					value.v6_0 := 0;
    					value.v6_1 := 0;
    					is_some := call.w0.%X0;
    					IF is_some THEN
    						value.kind := BYTE_TO_USINT(call.w0.%B2);
    						CASE value.kind OF
    							Wasi.Example.Types.ADDRESS_KIND#V4:
    								value.v4_0 := BYTE_TO_USINT(call.w0.%B4);
    								value.v4_1 := BYTE_TO_USINT(call.w0.%B5);
    								value.v4_2 := BYTE_TO_USINT(call.w0.%B6);
    								value.v4_3 := BYTE_TO_USINT(call.w0.%B7);
    							Wasi.Example.Types.ADDRESS_KIND#V6:
    								value.v6_0 := WORD_TO_UINT(call.w0.%W2);
    								value.v6_1 := WORD_TO_UINT(call.w0.%W3);
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
/// canonical ABI returns as one value, not in memory, which is the one
/// output it holds.
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

    			TYPE WRAPPED : STRUCT
    				inner_0 : UDINT;
    			END_STRUCT END_TYPE

    			/* The cases of `found`. */
    			TYPE FOUND_KIND : USINT (
    				NOTHING := 0,
    				BYTES := 1,
    				SIZE := 2
    			); END_TYPE

    			TYPE FOUND : STRUCT
    				kind : FOUND_KIND;
    				size : ULINT;
    			END_STRUCT END_TYPE

    			/* The cases of `choice`. */
    			TYPE CHOICE_KIND : USINT (
    				VALUE := 0,
    				NONE := 1
    			); END_TYPE

    			TYPE CHOICE : STRUCT
    				kind : CHOICE_KIND;
    				value : UDINT;
    			END_STRUCT END_TYPE

    			/* `find` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FIND` reads. */
    			{extern 'wasi:example/unread@0.3.0' 'find'}
    			FUNCTION PRIVATE FIND_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FIND : FOUND
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				FIND_RAW(w0 => w0, w1 => w1);
    				FIND.size := 0;
    				FIND.kind := BYTE_TO_USINT(w0.%B0);
    				CASE FIND.kind OF
    					FOUND_KIND#SIZE:
    						FIND.size := LWORD_TO_ULINT(w1);
    				END_CASE;
    			END_FUNCTION

    			/* `choose` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `CHOOSE` reads. */
    			{extern 'wasi:example/unread@0.3.0' 'choose'}
    			FUNCTION PRIVATE CHOOSE_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION CHOOSE : CHOICE
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				CHOOSE_RAW(w0 => w0, w1 => w1);
    				CHOOSE.value := 0;
    				CHOOSE.kind := BYTE_TO_USINT(w0.%B0);
    				CASE CHOOSE.kind OF
    					CHOICE_KIND#VALUE:
    						CHOOSE.value := DWORD_TO_UDINT(w0.%D1);
    				END_CASE;
    			END_FUNCTION

    			/* `wrap` as the host gives it: the one value of its result, which `WRAP`
    			   puts in its member. */
    			{extern 'wasi:example/unread@0.3.0' 'wrap'}
    			FUNCTION PRIVATE WRAP_RAW
    			VAR_OUTPUT
    				inner_0 : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WRAP : WRAPPED
    				WRAP_RAW(inner_0 => WRAP.inner_0);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `bytes` of `found` holds: a list, which an import cannot give
         back yet. */
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
        "TYPE BIG_KIND : UINT (",
        "PICK.kind := WORD_TO_UINT(w0.%W0);",
        "BIG_KIND#LAST:",
        "PICK.last := DWORD_TO_UDINT(w0.%D1);",
    ] {
        assert!(text.contains(line), "`{line}` in:\n{text}");
    }
}

/// A variant passed in is its case, then what each case holds, which a
/// call may leave out. As values, the cases share them, each as wide as the
/// widest any case puts there: a REAL and a ULINT in one LWORD. In memory,
/// where an async call's parameters go past 4 values, the variant is the
/// bytes of its place in the tuple, in words; a scalar and a string beside
/// it are passed as they are, and where the words are 4 at most a fifth
/// makes the call pass them in memory.
#[rstest]
fn a_variant_passed_in_is_packed_into_words(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface passing {
            record point { x: s32, y: s32 }
            variant target { nowhere, at(point), level(f32), far(u64) }
            aim: func(t: target);
            aim-later: async func(t: target) -> bool;
            aim-named: async func(id: u8, t: target, name: string);
            aim-both: async func(first: target, second: target);
        }

        world example {
            import passing;
        }
    "#), @r"
    // wasi-example-passing.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Passing
    			USING Std.Convert;

    			TYPE POINT : STRUCT
    				x : DINT;
    				y : DINT;
    			END_STRUCT END_TYPE

    			/* The cases of `target`. */
    			TYPE TARGET_KIND : USINT (
    				NOWHERE := 0,
    				AT_ := 1,
    				LEVEL := 2,
    				FAR := 3
    			); END_TYPE

    			TYPE TARGET : STRUCT
    				kind : TARGET_KIND;
    				at_ : POINT;
    				level : REAL;
    				far : ULINT;
    			END_STRUCT END_TYPE

    			/* `aim` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `AIM` packs. */
    			{extern 'wasi:example/passing@0.3.0' 'aim'}
    			FUNCTION PRIVATE AIM_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : LWORD;
    				a2 : DWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION AIM
    			VAR_INPUT
    				t : TARGET;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : LWORD;
    				a2 : DWORD;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a2 := 0;
    				a0.%B0 := USINT_TO_BYTE(t.kind);
    				CASE t.kind OF
    					TARGET_KIND#AT_:
    						a1.%D0 := DINT_TO_DWORD(t.at_.x);
    						a2 := DINT_TO_DWORD(t.at_.y);
    					TARGET_KIND#LEVEL:
    						a1.%D0 := REAL_TO_DWORD(t.level);
    					TARGET_KIND#FAR:
    						a1 := ULINT_TO_LWORD(t.far);
    				END_CASE;
    				AIM_RAW(a0 := a0, a1 := a1, a2 := a2);
    			END_FUNCTION

    			/* `aim-later` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `AIM_LATER` packs. */
    			{extern async 'wasi:example/passing@0.3.0' 'aim-later'}
    			FUNCTION_BLOCK AIM_LATER_RAW
    			VAR_INPUT
    				a0 : DWORD;
    				a1 : LWORD;
    				a2 : DWORD;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK AIM_LATER
    			VAR_INPUT
    				t : TARGET;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : BOOL;
    			END_VAR
    			VAR
    				a0 : DWORD;
    				a1 : LWORD;
    				a2 : DWORD;
    				call : AIM_LATER_RAW;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a2 := 0;
    				a0.%B0 := USINT_TO_BYTE(t.kind);
    				CASE t.kind OF
    					TARGET_KIND#AT_:
    						a1.%D0 := DINT_TO_DWORD(t.at_.x);
    						a2 := DINT_TO_DWORD(t.at_.y);
    					TARGET_KIND#LEVEL:
    						a1.%D0 := REAL_TO_DWORD(t.level);
    					TARGET_KIND#FAR:
    						a1 := ULINT_TO_LWORD(t.far);
    				END_CASE;
    				call(a0 := a0, a1 := a1, a2 := a2);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value := call.value;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* `aim-named` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `AIM_NAMED` packs. */
    			{extern async 'wasi:example/passing@0.3.0' 'aim-named'}
    			FUNCTION_BLOCK AIM_NAMED_RAW
    			VAR_INPUT
    				id : USINT;
    				a0 : LWORD;
    				a1 : LWORD;
    				name : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK AIM_NAMED
    			VAR_INPUT
    				id : USINT;
    				t : TARGET;
    				name : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				a0 : LWORD;
    				a1 : LWORD;
    				call : AIM_NAMED_RAW;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a0.%B0 := USINT_TO_BYTE(t.kind);
    				CASE t.kind OF
    					TARGET_KIND#AT_:
    						a1.%D0 := DINT_TO_DWORD(t.at_.x);
    						a1.%D1 := DINT_TO_DWORD(t.at_.y);
    					TARGET_KIND#LEVEL:
    						a1.%D0 := REAL_TO_DWORD(t.level);
    					TARGET_KIND#FAR:
    						a1 := ULINT_TO_LWORD(t.far);
    				END_CASE;
    				call(id := id, a0 := a0, a1 := a1, name := name);
    				DONE := call.DONE;
    			END_FUNCTION_BLOCK

    			/* `aim-both` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `AIM_BOTH` packs. */
    			{extern async 'wasi:example/passing@0.3.0' 'aim-both'}
    			FUNCTION_BLOCK AIM_BOTH_RAW
    			VAR_INPUT
    				a0 : LWORD;
    				a1 : LWORD;
    				a2 : LWORD;
    				a3 : LWORD;
    				a4 : LWORD;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK AIM_BOTH
    			VAR_INPUT
    				first : TARGET;
    				second : TARGET;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				a0 : LWORD;
    				a1 : LWORD;
    				a2 : LWORD;
    				a3 : LWORD;
    				a4 : LWORD;
    				call : AIM_BOTH_RAW;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a2 := 0;
    				a3 := 0;
    				a4 := 0;
    				a0.%B0 := USINT_TO_BYTE(first.kind);
    				CASE first.kind OF
    					TARGET_KIND#AT_:
    						a1.%D0 := DINT_TO_DWORD(first.at_.x);
    						a1.%D1 := DINT_TO_DWORD(first.at_.y);
    					TARGET_KIND#LEVEL:
    						a1.%D0 := REAL_TO_DWORD(first.level);
    					TARGET_KIND#FAR:
    						a1 := ULINT_TO_LWORD(first.far);
    				END_CASE;
    				a2.%B0 := USINT_TO_BYTE(second.kind);
    				CASE second.kind OF
    					TARGET_KIND#AT_:
    						a3.%D0 := DINT_TO_DWORD(second.at_.x);
    						a3.%D1 := DINT_TO_DWORD(second.at_.y);
    					TARGET_KIND#LEVEL:
    						a3.%D0 := REAL_TO_DWORD(second.level);
    					TARGET_KIND#FAR:
    						a3 := ULINT_TO_LWORD(second.far);
    				END_CASE;
    				call(a0 := a0, a1 := a1, a2 := a2, a3 := a3, a4 := a4);
    				DONE := call.DONE;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
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
    VAR_OUTPUT value : Wasi.Example.Shapes.SHAPE; END_VAR
    VAR count : DINT; END_VAR
        value := Wasi.Example.Shapes.PICK(which := which);
        count := 0;
        IF value.circle <> 0.0 THEN count := count + 1; END_IF;
        IF value.square <> 0 THEN count := count + 1; END_IF;
        IF value.at_.x <> 0 THEN count := count + 1; END_IF;
        IF value.at_.y <> 0 THEN count := count + 1; END_IF;
        IF value.pair_0 <> 0 THEN count := count + 1; END_IF;
        IF value.pair_1 <> 0 THEN count := count + 1; END_IF;
        IF value.tinted <> 0 THEN count := count + 1; END_IF;
        IF value.marked <> 0 THEN count := count + 1; END_IF;
        IF value.wide <> 0.0 THEN count := count + 1; END_IF;
        IF value.signed <> 0 THEN count := count + 1; END_IF;
        IF value.long <> 0 THEN count := count + 1; END_IF;
        picked := value.kind = shape AND count = nonzero;
    END_FUNCTION

    FUNCTION check : DINT
    VAR
        ok, some, failed : BOOL;
        shape : Wasi.Example.Shapes.SHAPE;
        error : Wasi.Example.Shapes.ERROR_CODE;
        inner : Wasi.Example.Shapes.OUTER;
        count : ULINT; level : USINT;
    END_VAR
        check := 0;
        // `at`, then `circle`: what `at` held is gone.
        ok := picked(which := 3, shape := 3, nonzero := 2, value => shape);
        IF NOT ok OR shape.at_.x <> -1 OR shape.at_.y <> 2 THEN check := -1; RETURN; END_IF;
        ok := picked(which := 1, shape := 1, nonzero := 1, value => shape);
        IF NOT ok OR shape.circle <> 1.5 THEN check := -2; RETURN; END_IF;
        ok := picked(which := 4, shape := 4, nonzero := 2, value => shape);
        IF NOT ok OR shape.pair_0 <> -5 OR shape.pair_1 <> ULINT#16#0102030405060708 THEN
            check := -3; RETURN;
        END_IF;
        ok := picked(which := 2, shape := 2, nonzero := 1, value => shape);
        IF NOT ok OR shape.square <> 65535 THEN check := -4; RETURN; END_IF;
        ok := picked(which := 0, shape := 0, nonzero := 0, value => shape);
        IF NOT ok THEN check := -5; RETURN; END_IF;
        ok := picked(which := 5, shape := 5, nonzero := 1, value => shape);
        IF NOT ok OR shape.tinted <> 2 THEN check := -6; RETURN; END_IF;
        ok := picked(which := 6, shape := 6, nonzero := 1, value => shape);
        IF NOT ok OR shape.marked <> 16#05 THEN check := -7; RETURN; END_IF;
        ok := picked(which := 7, shape := 7, nonzero := 1, value => shape);
        IF NOT ok OR shape.wide <> -2.25 THEN check := -8; RETURN; END_IF;
        ok := picked(which := 8, shape := 8, nonzero := 1, value => shape);
        IF NOT ok OR shape.signed <> -300 THEN check := -9; RETURN; END_IF;
        ok := picked(which := 9, shape := 9, nonzero := 1, value => shape);
        IF NOT ok OR shape.long <> LINT#-1099511627776 THEN check := -10; RETURN; END_IF;
        // A case whose value is a string comes back as its name alone.
        ok := picked(which := 10, shape := 10, nonzero := 0, value => shape);
        IF NOT ok THEN check := -11; RETURN; END_IF;

        Wasi.Example.Shapes.FIND(which := 1, is_some => some, value => shape);
        IF NOT some OR shape.kind <> 1 OR shape.circle <> 2.5 THEN check := -12; RETURN; END_IF;
        Wasi.Example.Shapes.FIND(which := 2, is_some => some, value => shape);
        IF NOT some OR shape.kind <> 3 OR shape.at_.x <> 7 OR shape.at_.y <> -8
            OR shape.circle <> 0.0
        THEN
            check := -13; RETURN;
        END_IF;
        Wasi.Example.Shapes.FIND(which := 0, is_some => some, value => shape);
        IF some OR shape.kind <> 0 OR shape.at_.x <> 0 OR shape.at_.y <> 0 THEN
            check := -14; RETURN;
        END_IF;

        Wasi.Example.Shapes.MEASURE(which := 0, failed => failed, value => shape, error => error);
        IF failed OR shape.kind <> 2 OR shape.square <> 9 OR error.kind <> 0 THEN
            check := -15; RETURN;
        END_IF;
        Wasi.Example.Shapes.MEASURE(which := 1, failed => failed, value => shape, error => error);
        IF NOT failed OR error.kind <> 1 OR shape.kind <> 0 OR shape.square <> 0 THEN
            check := -16; RETURN;
        END_IF;
        Wasi.Example.Shapes.MEASURE(which := 2, failed => failed, error => error);
        IF NOT failed OR error.kind <> 2 THEN check := -17; RETURN; END_IF;

        Wasi.Example.Shapes.COUNT(which := 0, failed => failed, value => count, error => error);
        IF failed OR count <> ULINT#18446744073709551614 OR error.kind <> 0 THEN
            check := -18; RETURN;
        END_IF;
        Wasi.Example.Shapes.COUNT(which := 1, failed => failed, value => count, error => error);
        IF NOT failed OR error.kind <> 1 OR count <> 0 THEN check := -19; RETURN; END_IF;

        Wasi.Example.Shapes.LEVEL(which := 0, failed => failed, value => level, error => error);
        IF failed OR level <> 200 OR error.kind <> 0 THEN check := -20; RETURN; END_IF;
        Wasi.Example.Shapes.LEVEL(which := 1, failed => failed, value => level, error => error);
        IF NOT failed OR error.kind <> 1 OR level <> 0 THEN check := -21; RETURN; END_IF;

        // A record holding an option and a tuple, in members of its STRUCT.
        inner := Wasi.Example.Shapes.INNER(which := 0);
        IF inner.a <> 9 OR NOT inner.at_is_some OR inner.at_.x <> 3 OR inner.at_.y <> -4
            OR NOT inner.tail_0 OR inner.tail_1 <> 77
        THEN
            check := -22; RETURN;
        END_IF;
        inner := Wasi.Example.Shapes.INNER(which := 1);
        IF inner.a <> 1 OR inner.at_is_some OR inner.at_.x <> 0 OR inner.at_.y <> 0
            OR inner.tail_0 OR inner.tail_1 <> UDINT#4000000000
        THEN
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
                IF wait.failed OR wait.value.x <> 5 OR wait.value.y <> -6 OR wait.error.kind <> 0 THEN
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
                IF NOT wait.failed OR wait.error.kind <> 1 OR wait.value.x <> 0
                    OR wait.value.y <> 0
                THEN
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

/// Values a host gives back as it took them: a variant of every kind of
/// case, an option and a result, each also of a float alone, a record of
/// records, as values and in memory.
const ECHOES: &str = r#"
    package wasi:example@0.3.0;

    interface echoes {
        record point { x: s32, y: s32 }
        enum color { red, green, blue }
        flags marks { bold, dashed, filled }
        variant figure {
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
            flagged(bool),
        }
        record segment { start: point, end: point, width: u8 }
        echo: func(f: figure) -> figure;
        echo-later: async func(f: figure) -> figure;
        echo-two: async func(first: figure, id: u32, second: figure) -> tuple<figure, u32, figure>;
        maybe: func(p: option<point>) -> option<point>;
        outcome: func(r: result<u32, color>) -> result<u32, color>;
        warm: func(t: option<f32>) -> option<f32>;
        reach: func(d: result<f64, f64>) -> result<f64, f64>;
        line: func(s: segment) -> segment;
        line-later: async func(s: segment, scale: u8) -> segment;
        tagged: async func(name: string, f: figure, id: u32) -> tuple<u32, figure>;
    }

    world example {
        import echoes;
    }
"#;

/// The host's side of [`ECHOES`]: each function gives back what it took,
/// `echo-two` its three parameters as a tuple, `line-later` its segment as
/// wide as `scale`, `tagged` its figure after `id` plus the length of
/// `name`.
fn echoes_host(linker: &mut Linker<Host>) {
    let mut echoes = linker
        .instance("wasi:example/echoes@0.3.0")
        .expect("the interface");
    for name in ["echo", "maybe", "outcome", "warm", "reach", "line"] {
        echoes
            .func_new(name, |_, _, params, results| {
                results[0] = params[0].clone();
                Ok(())
            })
            .expect("a function of the host");
    }
    echoes
        .func_new_concurrent("echo-later", |_, _, params, results| {
            Box::pin(async move {
                Later(false).await;
                results[0] = params[0].clone();
                Ok(())
            })
        })
        .expect("an async function of the host");
    echoes
        .func_new_concurrent("echo-two", |_, _, params, results| {
            Box::pin(async move {
                Later(false).await;
                results[0] = Val::Tuple(params.to_vec());
                Ok(())
            })
        })
        .expect("an async function of the host");
    echoes
        .func_new_concurrent("line-later", |_, _, params, results| {
            Box::pin(async move {
                let [Val::Record(fields), scale] = params else {
                    panic!("a segment and a scale: {params:?}");
                };
                let fields = fields
                    .iter()
                    .map(|(name, value)| match name.as_str() {
                        "width" => (name.clone(), scale.clone()),
                        _ => (name.clone(), value.clone()),
                    })
                    .collect();
                results[0] = Val::Record(fields);
                Ok(())
            })
        })
        .expect("an async function of the host");
    echoes
        .func_new_concurrent("tagged", |_, _, params, results| {
            Box::pin(async move {
                let [Val::String(name), figure, Val::U32(id)] = params else {
                    panic!("a name, a figure and a number: {params:?}");
                };
                results[0] = Val::Tuple(vec![Val::U32(id + name.len() as u32), figure.clone()]);
                Ok(())
            })
        })
        .expect("an async function of the host");
}

/// What the host of [`ECHOES`] gives back, compared with what it was
/// passed: 1 when every value came back as it went and nothing else did,
/// and otherwise the number of the check that failed, negative.
const ECHOED: &str = r#"
    /* A figure of the case `kind`, every value of it at zero. */
    FUNCTION figure : Wasi.Example.Echoes.FIGURE
    VAR_INPUT kind : USINT; END_VAR
        figure.kind := kind;
    END_FUNCTION

    /* Whether a figure comes back as it was passed: its case, what the case
       holds, and zero for what the other cases would hold. */
    FUNCTION same : BOOL
    VAR_INPUT f : Wasi.Example.Echoes.FIGURE; END_VAR
    VAR got : Wasi.Example.Echoes.FIGURE; END_VAR
        got := Wasi.Example.Echoes.ECHO(f := f);
        same := got.kind = f.kind AND got.circle = f.circle AND got.square = f.square
            AND got.at_.x = f.at_.x AND got.at_.y = f.at_.y AND got.pair_0 = f.pair_0
            AND got.pair_1 = f.pair_1 AND got.tinted = f.tinted AND got.marked = f.marked
            AND got.wide = f.wide AND got.signed = f.signed AND got.long = f.long
            AND got.flagged = f.flagged;
    END_FUNCTION

    FUNCTION check : DINT
    VAR
        some, failed : BOOL;
        f, got : Wasi.Example.Echoes.FIGURE;
        point : Wasi.Example.Echoes.POINT;
        segment : Wasi.Example.Echoes.SEGMENT;
        value : UDINT;
        error : Wasi.Example.Echoes.COLOR;
        circle : REAL;
        near, far : LREAL;
    END_VAR
        check := 0;
        IF NOT same(f := figure(0)) THEN check := -1; RETURN; END_IF;
        f := figure(1); f.circle := 1.5;
        IF NOT same(f := f) THEN check := -2; RETURN; END_IF;
        f := figure(2); f.square := 65535;
        IF NOT same(f := f) THEN check := -3; RETURN; END_IF;
        f := figure(3); f.at_.x := -1; f.at_.y := 2;
        IF NOT same(f := f) THEN check := -4; RETURN; END_IF;
        f := figure(4); f.pair_0 := -5; f.pair_1 := ULINT#16#0102030405060708;
        IF NOT same(f := f) THEN check := -5; RETURN; END_IF;
        f := figure(5); f.tinted := 2;
        IF NOT same(f := f) THEN check := -6; RETURN; END_IF;
        f := figure(6); f.marked := 16#05;
        IF NOT same(f := f) THEN check := -7; RETURN; END_IF;
        f := figure(7); f.wide := -2.25;
        IF NOT same(f := f) THEN check := -8; RETURN; END_IF;
        f := figure(8); f.signed := -300;
        IF NOT same(f := f) THEN check := -9; RETURN; END_IF;
        f := figure(9); f.long := LINT#-1099511627776;
        IF NOT same(f := f) THEN check := -10; RETURN; END_IF;
        // A flag where the case before held a number: FALSE is every bit of
        // it at zero, which the words start from at each call.
        f := figure(10); f.flagged := FALSE;
        IF NOT same(f := f) THEN check := -17; RETURN; END_IF;
        f := figure(10); f.flagged := TRUE;
        IF NOT same(f := f) THEN check := -18; RETURN; END_IF;
        // What another case would hold is not passed with this one.
        f := figure(1); f.circle := 2.5; f.at_.x := 7; f.at_.y := 8; f.long := 9;
        got := Wasi.Example.Echoes.ECHO(f := f);
        IF got.kind <> 1 OR got.circle <> 2.5 OR got.at_.x <> 0 OR got.at_.y <> 0 OR got.long <> 0
        THEN
            check := -11; RETURN;
        END_IF;

        point.x := 7; point.y := -8;
        Wasi.Example.Echoes.MAYBE(p_is_some := TRUE, p_ := point, is_some => some, value => point);
        IF NOT some OR point.x <> 7 OR point.y <> -8 THEN check := -12; RETURN; END_IF;
        Wasi.Example.Echoes.MAYBE(p_is_some := FALSE, p_ := point, is_some => some, value => point);
        IF some OR point.x <> 0 OR point.y <> 0 THEN check := -13; RETURN; END_IF;

        Wasi.Example.Echoes.OUTCOME(r_failed := FALSE, r_value := 4000000000,
            failed => failed, value => value, error => error);
        IF failed OR value <> 4000000000 OR error <> 0 THEN check := -14; RETURN; END_IF;
        Wasi.Example.Echoes.OUTCOME(r_failed := TRUE, r_value := 77, r_error := 2,
            failed => failed, value => value, error => error);
        IF NOT failed OR value <> 0 OR error <> 2 THEN check := -15; RETURN; END_IF;

        // A record of records, passed field by field and given back whole.
        segment.start.x := 1; segment.start.y := -2; segment.end.x := 3; segment.end.y := -4;
        segment.width := 200;
        segment := Wasi.Example.Echoes.LINE(s_ := segment);
        IF segment.start.x <> 1 OR segment.start.y <> -2 OR segment.end.x <> 3
            OR segment.end.y <> -4 OR segment.width <> 200
        THEN
            check := -16; RETURN;
        END_IF;

        // A value both sides agree on is passed as the REAL or the LREAL it is.
        Wasi.Example.Echoes.WARM(t_is_some := TRUE, t := 36.5, is_some => some,
            value => circle);
        IF NOT some OR circle <> 36.5 THEN check := -26; RETURN; END_IF;
        Wasi.Example.Echoes.WARM(t_is_some := FALSE, t := 36.5, is_some => some);
        IF some THEN check := -27; RETURN; END_IF;
        Wasi.Example.Echoes.REACH(d_failed := FALSE, d_value := -2.5,
            failed => failed, value => near, error => far);
        IF failed OR near <> -2.5 OR far <> 0.0 THEN check := -28; RETURN; END_IF;
        Wasi.Example.Echoes.REACH(d_failed := TRUE, d_value := 1.0, d_error := 1.0E300,
            failed => failed, value => near, error => far);
        IF NOT failed OR near <> 0.0 OR far <> 1.0E300 THEN check := -29; RETURN; END_IF;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR
        checked : BOOL;
        step : INT;
        first, second : Wasi.Example.Echoes.FIGURE;
        segment : Wasi.Example.Echoes.SEGMENT;
        later : Wasi.Example.Echoes.ECHO_LATER;
        two : Wasi.Example.Echoes.ECHO_TWO;
        line : Wasi.Example.Echoes.LINE_LATER;
        tagged : Wasi.Example.Echoes.TAGGED;
    END_VAR
        IF NOT checked THEN
            checked := TRUE;
            output := check();
            IF output <> 0 THEN step := 9; END_IF;
        END_IF;
        CASE step OF
        0:
            first := figure(4); first.pair_0 := -5; first.pair_1 := ULINT#16#0102030405060708;
            later(f := first);
            IF later.DONE THEN
                IF later.value.kind <> 4 OR later.value.pair_0 <> -5
                    OR later.value.pair_1 <> ULINT#16#0102030405060708
                THEN
                    output := -20; step := 9;
                ELSE
                    step := 1;
                END_IF;
            END_IF;
        1:
            // The pair the block still holds is not passed with this case.
            first := figure(7); first.wide := -2.25;
            later(f := first);
            IF later.DONE THEN
                IF later.value.kind <> 7 OR later.value.wide <> -2.25 OR later.value.pair_0 <> 0
                    OR later.value.pair_1 <> 0
                THEN
                    output := -21; step := 9;
                ELSE
                    step := 2;
                END_IF;
            END_IF;
        2:
            first := figure(3); first.at_.x := -1; first.at_.y := 2;
            second := figure(9); second.long := LINT#-1099511627776;
            two(first := first, id := 4000000000, second := second);
            IF two.DONE THEN
                IF two.item0.kind <> 3 OR two.item0.at_.x <> -1 OR two.item0.at_.y <> 2
                    OR two.item1 <> 4000000000 OR two.item2.kind <> 9
                    OR two.item2.long <> LINT#-1099511627776
                THEN
                    output := -22; step := 9;
                ELSE
                    step := 3;
                END_IF;
            END_IF;
        3:
            first := figure(6); first.marked := 16#05;
            second := figure(2); second.square := 65535;
            two(first := first, id := 7, second := second);
            IF two.DONE THEN
                IF two.item0.kind <> 6 OR two.item0.marked <> 16#05 OR two.item0.at_.x <> 0
                    OR two.item1 <> 7 OR two.item2.kind <> 2 OR two.item2.square <> 65535
                    OR two.item2.long <> 0
                THEN
                    output := -23; step := 9;
                ELSE
                    step := 4;
                END_IF;
            END_IF;
        4:
            segment.start.x := 1; segment.start.y := -2; segment.end.x := 3;
            segment.end.y := -4; segment.width := 5;
            line(s_ := segment, scale := 9);
            IF line.DONE THEN
                IF line.value.start.x <> 1 OR line.value.start.y <> -2 OR line.value.end.x <> 3
                    OR line.value.end.y <> -4 OR line.value.width <> 9
                THEN
                    output := -24; step := 9;
                ELSE
                    step := 5;
                END_IF;
            END_IF;
        5:
            // A string beside a figure in the tuple.
            first := figure(2); first.square := 9;
            tagged(name := 'four', f := first, id := 100);
            IF tagged.DONE THEN
                IF tagged.item0 <> 104 OR tagged.item1.kind <> 2 OR tagged.item1.square <> 9 THEN
                    output := -25;
                ELSE
                    output := 1;
                END_IF;
                step := 9;
            END_IF;
        END_CASE;
    END_PROGRAM
"#;

/// What generated code packs for a host comes back from it as it went:
/// each case of a variant passed as values, the cases sharing them; an
/// option and a result, a float as the REAL or the LREAL it is where no
/// other value shares its place; a record of records; and, in an async call, a
/// variant as values and two of them in the tuple the call's parameters
/// are in, beside a scalar or a string. wasmtime reads what was packed, as
/// the canonical ABI says, and lays out what the host gives back.
#[rstest]
fn what_is_packed_for_a_host_comes_back_as_it_went(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, ECHOES, ECHOED, echoes_host);
    let mut output = 0;
    for _ in 0..40 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}
