// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The built-ins of streams and futures: one set per type in each
//! interface that takes or gives one, in its namespace, imported under the
//! name of its first function whose type has it, with the place the stream
//! or future has in that function's type, and named after the type and
//! what carries it: `BYTES_STREAM`, `RESULT_FUTURE`. A stream of bytes is
//! read into a STRING or an ARRAY OF BYTE and written from a STRING; a
//! stream of anything else is made and dropped only; a future's value comes
//! back as outputs, as a function's would.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{checked, generated};
use crate::tests::utils::with_db;

/// A set per type in each interface: the two functions of one interface
/// that take or give a stream of bytes share one, named after the first;
/// another interface has its own, named after its own function.
#[rstest]
fn one_set_per_type_across_functions_and_interfaces(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface pipe {
            enum error-code { closed, full }
            send: func(data: stream<u8>) -> future<result<_, error-code>>;
            receive: func() -> tuple<stream<u8>, future<result<_, error-code>>>;
        }

        interface other {
            use pipe.{error-code};
            relay: func(data: stream<u8>) -> future<result<_, error-code>>;
        }

        world example {
            import pipe;
            import other;
        }
    "#), @r"
    // wasi-example-pipe.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Pipe
    			TYPE ERROR_CODE : USINT (
    				CLOSED := 0,
    				FULL := 1
    			); END_TYPE

    			{extern 'wasi:example/pipe@0.3.0' 'send'}
    			FUNCTION SEND : UDINT
    			VAR_INPUT
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'wasi:example/pipe@0.3.0' '[stream-new-0]send'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/pipe@0.3.0' '[stream-read-0]send'}
    			FUNCTION_BLOCK BYTES_STREAM_READ
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : STRING[4096];
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/pipe@0.3.0' '[stream-read-0]send'}
    			FUNCTION_BLOCK BYTES_STREAM_READ_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..4095] OF BYTE;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'wasi:example/pipe@0.3.0' '[stream-write-0]send'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE
    			VAR_INPUT
    				stream : UDINT;
    				data : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/pipe@0.3.0' '[stream-drop-readable-0]send'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/pipe@0.3.0' '[stream-drop-writable-0]send'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a future of `result<_, error-code>`, and gives its reading end,
    			   then its writing end. */
    			{extern 'wasi:example/pipe@0.3.0' '[future-new-1]send'}
    			FUNCTION RESULT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/pipe@0.3.0' '[future-read-1]send'}
    			FUNCTION_BLOCK RESULT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/pipe@0.3.0' '[future-drop-readable-1]send'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/pipe@0.3.0' '[future-drop-writable-1]send'}
    			FUNCTION RESULT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/pipe@0.3.0' 'receive'}
    			FUNCTION RECEIVE
    			VAR_OUTPUT
    				item0 : UDINT;
    				item1 : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_FUTURE_WRITE`: a future's write is not imported yet.
       - `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan. */

    // wasi-example-other.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Other
    			{extern 'wasi:example/other@0.3.0' 'relay'}
    			FUNCTION RELAY : UDINT
    			VAR_INPUT
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'wasi:example/other@0.3.0' '[stream-new-0]relay'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/other@0.3.0' '[stream-read-0]relay'}
    			FUNCTION_BLOCK BYTES_STREAM_READ
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : STRING[4096];
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/other@0.3.0' '[stream-read-0]relay'}
    			FUNCTION_BLOCK BYTES_STREAM_READ_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..4095] OF BYTE;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'wasi:example/other@0.3.0' '[stream-write-0]relay'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE
    			VAR_INPUT
    				stream : UDINT;
    				data : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/other@0.3.0' '[stream-drop-readable-0]relay'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/other@0.3.0' '[stream-drop-writable-0]relay'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a future of `result<_, error-code>`, and gives its reading end,
    			   then its writing end. */
    			{extern 'wasi:example/other@0.3.0' '[future-new-1]relay'}
    			FUNCTION RESULT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/other@0.3.0' '[future-read-1]relay'}
    			FUNCTION_BLOCK RESULT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : Wasi.Example.Pipe.ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/other@0.3.0' '[future-drop-readable-1]relay'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/other@0.3.0' '[future-drop-writable-1]relay'}
    			FUNCTION RESULT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_FUTURE_WRITE`: a future's write is not imported yet.
       - `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}

/// The place in a function's type counts streams and futures together, in
/// the order WIT walks the type: the second is `-1`. A method's built-ins
/// are named after the method.
#[rstest]
fn the_place_counts_streams_and_futures_together(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface copy {
            resource channel {
                transfer: func(from-here: future<u32>, data: stream<u8>) -> bool;
            }
        }

        world example {
            import copy;
        }
    "#), @r"
    // wasi-example-copy.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Copy
    			/* Lets go of `handle`, a handle of `channel` the host gave. */
    			{extern 'wasi:example/copy@0.3.0' '[resource-drop]channel'}
    			FUNCTION CHANNEL_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/copy@0.3.0' '[method]channel.transfer'}
    			FUNCTION CHANNEL_TRANSFER : BOOL
    			VAR_INPUT
    				self : UDINT;
    				from_here : UDINT;
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a future of `u32`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/copy@0.3.0' '[future-new-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/copy@0.3.0' '[future-read-0][method]channel.transfer'}
    			FUNCTION_BLOCK U32_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/copy@0.3.0' '[future-drop-readable-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/copy@0.3.0' '[future-drop-writable-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'wasi:example/copy@0.3.0' '[stream-new-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/copy@0.3.0' '[stream-read-1][method]channel.transfer'}
    			FUNCTION_BLOCK BYTES_STREAM_READ
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : STRING[4096];
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:example/copy@0.3.0' '[stream-read-1][method]channel.transfer'}
    			FUNCTION_BLOCK BYTES_STREAM_READ_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..4095] OF BYTE;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'wasi:example/copy@0.3.0' '[stream-write-1][method]channel.transfer'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE
    			VAR_INPUT
    				stream : UDINT;
    				data : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/copy@0.3.0' '[stream-drop-readable-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/copy@0.3.0' '[stream-drop-writable-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_FUTURE_WRITE`: a future's write is not imported yet.
       - `U32_FUTURE_CANCEL_READ` and `U32_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}

/// A stream of anything but bytes is made and dropped, not read or written
/// yet: of a number, of a record, of a resource's handles.
#[rstest]
fn a_stream_of_anything_else_is_made_and_dropped(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface feeds {
            record event { code: u32 }
            resource connection;
            numbers: func() -> stream<u32>;
            events: func() -> stream<event>;
            accept: func() -> stream<connection>;
        }

        world example {
            import feeds;
        }
    "#), @r"
    // wasi-example-feeds.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Feeds
    			/* Lets go of `handle`, a handle of `connection` the host gave. */
    			{extern 'wasi:example/feeds@0.3.0' '[resource-drop]connection'}
    			FUNCTION CONNECTION_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/feeds@0.3.0' 'numbers'}
    			FUNCTION NUMBERS : UDINT
    			END_FUNCTION

    			/* Makes a stream of `u32`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]numbers'}
    			FUNCTION U32_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]numbers'}
    			FUNCTION U32_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]numbers'}
    			FUNCTION U32_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/feeds@0.3.0' 'events'}
    			FUNCTION EVENTS : UDINT
    			END_FUNCTION

    			/* Makes a stream of `event`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]events'}
    			FUNCTION EVENT_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]events'}
    			FUNCTION EVENT_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]events'}
    			FUNCTION EVENT_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/feeds@0.3.0' 'accept'}
    			FUNCTION ACCEPT : UDINT
    			END_FUNCTION

    			/* Makes a stream of `own<connection>`, and gives its reading end, then its
    			   writing end. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]accept'}
    			FUNCTION CONNECTION_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]accept'}
    			FUNCTION CONNECTION_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]accept'}
    			FUNCTION CONNECTION_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_STREAM_READ` and `U32_STREAM_WRITE`: a stream's read or write
         moves bytes only.
       - `U32_STREAM_CANCEL_READ` and `U32_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `EVENT_STREAM_READ` and `EVENT_STREAM_WRITE`: a stream's read or write
         moves bytes only.
       - `EVENT_STREAM_CANCEL_READ` and `EVENT_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `CONNECTION_STREAM_READ` and `CONNECTION_STREAM_WRITE`: a stream's
         read or write moves bytes only.
       - `CONNECTION_STREAM_CANCEL_READ` and `CONNECTION_STREAM_CANCEL_WRITE`:
         a cancel blocks the scan. */
    ");
}

/// A future's value comes back as a function's would: nothing, a scalar,
/// a flag and a side of a result, a flag and an option's value, a record's
/// fields; a result with both sides in words, which its read is read out
/// of. A string is not read yet.
#[rstest]
fn a_future_s_value_comes_back_as_outputs(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface promises {
            enum error-code { lost }
            record point { x: f64, y: f64 }
            ready: func() -> future;
            count: func() -> future<u32>;
            outcome: func() -> future<result<_, error-code>>;
            maybe: func() -> future<option<s64>>;
            where-is: func() -> future<point>;
            name: func() -> future<string>;
            both: func() -> future<result<u64, error-code>>;
        }

        world example {
            import promises;
        }
    "#), @r"
    // wasi-example-promises.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Promises
    			USING Std.Convert;

    			TYPE ERROR_CODE : USINT (
    				LOST := 0
    			); END_TYPE

    			{extern 'wasi:example/promises@0.3.0' 'ready'}
    			FUNCTION READY : UDINT
    			END_FUNCTION

    			/* Makes a future of nothing, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]ready'}
    			FUNCTION UNIT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]ready'}
    			FUNCTION_BLOCK UNIT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]ready'}
    			FUNCTION UNIT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]ready'}
    			FUNCTION UNIT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'count'}
    			FUNCTION COUNT : UDINT
    			END_FUNCTION

    			/* Makes a future of `u32`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]count'}
    			FUNCTION U32_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]count'}
    			FUNCTION_BLOCK U32_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]count'}
    			FUNCTION U32_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]count'}
    			FUNCTION U32_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'outcome'}
    			FUNCTION OUTCOME : UDINT
    			END_FUNCTION

    			/* Makes a future of `result<_, error-code>`, and gives its reading end,
    			   then its writing end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]outcome'}
    			FUNCTION RESULT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]outcome'}
    			FUNCTION_BLOCK RESULT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]outcome'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]outcome'}
    			FUNCTION RESULT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'maybe'}
    			FUNCTION MAYBE : UDINT
    			END_FUNCTION

    			/* Makes a future of `option<s64>`, and gives its reading end, then its
    			   writing end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]maybe'}
    			FUNCTION OPTION_S64_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]maybe'}
    			FUNCTION_BLOCK OPTION_S64_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				is_some : BOOL;
    				value : LINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]maybe'}
    			FUNCTION OPTION_S64_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]maybe'}
    			FUNCTION OPTION_S64_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'where-is'}
    			FUNCTION WHERE_IS : UDINT
    			END_FUNCTION

    			/* Makes a future of `point`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]where-is'}
    			FUNCTION POINT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]where-is'}
    			FUNCTION_BLOCK POINT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				x : LREAL;
    				y : LREAL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]where-is'}
    			FUNCTION POINT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]where-is'}
    			FUNCTION POINT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'name'}
    			FUNCTION NAME : UDINT
    			END_FUNCTION

    			/* Makes a future of `string`, and gives its reading end, then its writing
    			   end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]name'}
    			FUNCTION STRING_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]name'}
    			FUNCTION STRING_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]name'}
    			FUNCTION STRING_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'both'}
    			FUNCTION BOTH : UDINT
    			END_FUNCTION

    			/* Makes a future of `result<u64, error-code>`, and gives its reading end,
    			   then its writing end. */
    			{extern 'wasi:example/promises@0.3.0' '[future-new-0]both'}
    			FUNCTION RESULT_U64_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* The future's read as the host gives it: its value in words of 8 bytes,
    			   as the canonical ABI lays it out, which `RESULT_U64_FUTURE_READ` reads. */
    			{extern async 'wasi:example/promises@0.3.0' '[future-read-0]both'}
    			FUNCTION_BLOCK RESULT_U64_FUTURE_READ_RAW
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			FUNCTION_BLOCK RESULT_U64_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				value : ULINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_U64_FUTURE_READ_RAW;
    			END_VAR
    				call(future := future);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value := 0;
    					error := 0;
    					failed := call.w0.%X0;
    					IF failed THEN
    						error := BYTE_TO_USINT(call.w1.%B0);
    					ELSE
    						value := LWORD_TO_ULINT(call.w1);
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]both'}
    			FUNCTION RESULT_U64_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]both'}
    			FUNCTION RESULT_U64_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `UNIT_FUTURE_WRITE`: a future's write is not imported yet.
       - `UNIT_FUTURE_CANCEL_READ` and `UNIT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `U32_FUTURE_WRITE`: a future's write is not imported yet.
       - `U32_FUTURE_CANCEL_READ` and `U32_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_FUTURE_WRITE`: a future's write is not imported yet.
       - `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `OPTION_S64_FUTURE_WRITE`: a future's write is not imported yet.
       - `OPTION_S64_FUTURE_CANCEL_READ` and `OPTION_S64_FUTURE_CANCEL_WRITE`:
         a cancel blocks the scan.
       - `POINT_FUTURE_WRITE`: a future's write is not imported yet.
       - `POINT_FUTURE_CANCEL_READ` and `POINT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `STRING_FUTURE_READ`: its value holds a string, which a read cannot
         give back yet.
       - `STRING_FUTURE_WRITE`: a future's write is not imported yet.
       - `STRING_FUTURE_CANCEL_READ` and `STRING_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_U64_FUTURE_WRITE`: a future's write is not imported yet.
       - `RESULT_U64_FUTURE_CANCEL_READ` and `RESULT_U64_FUTURE_CANCEL_WRITE`:
         a cancel blocks the scan. */
    ");
}

/// A built-in whose name a function of the interface takes stops the
/// generator, which would otherwise write two POUs of one name.
#[test]
fn a_built_in_named_like_a_function_stops_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        interface feeds {
            record event { x: u32 }
            event-stream-new: func() -> u32;
            feed: func() -> stream<event>;
        }

        world example {
            import feeds;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"`event-stream-new` and a built-in of `event` would both be `EVENT_STREAM_NEW` in `wasi:example/feeds@0.3.0`");
}

/// The built-ins of Std's standard output: a stream of bytes and the
/// future it answers with, named after its `write-via-stream`, as the
/// standard error's are after its own.
#[test]
fn std_s_built_ins() {
    let bindings = generated(rk_bindgen::STD_WORLD);
    let stdout = bindings
        .files
        .iter()
        .find(|file| file.name == "wasi-cli-stdout.st")
        .expect("stdout's file");
    assert_snapshot!(stdout.text, @r"
    /* Generated from the WIT of wasi:cli@0.3.0. DO NOT EDIT! */
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Cli
    		NAMESPACE Stdout
    			/* Write the given stream to stdout.

    			   If the stream's writable end is dropped this function will either return
    			   success once the entire contents of the stream have been written or an
    			   error-code representing a failure.

    			   Otherwise if there is an error the readable end of the stream will be
    			   dropped and this function will return an error-code. */
    			{extern 'wasi:cli/stdout@0.3.0' 'write-via-stream'}
    			FUNCTION WRITE_VIA_STREAM : UDINT
    			VAR_INPUT
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:cli/stdout@0.3.0' '[stream-read-0]write-via-stream'}
    			FUNCTION_BLOCK BYTES_STREAM_READ
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : STRING[4096];
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'wasi:cli/stdout@0.3.0' '[stream-read-0]write-via-stream'}
    			FUNCTION_BLOCK BYTES_STREAM_READ_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..4095] OF BYTE;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE
    			VAR_INPUT
    				stream : UDINT;
    				data : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'wasi:cli/stdout@0.3.0' '[stream-drop-readable-0]write-via-stream'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'wasi:cli/stdout@0.3.0' '[stream-drop-writable-0]write-via-stream'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a future of `result<_, error-code>`, and gives its reading end,
    			   then its writing end. */
    			{extern 'wasi:cli/stdout@0.3.0' '[future-new-1]write-via-stream'}
    			FUNCTION RESULT_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'wasi:cli/stdout@0.3.0' '[future-read-1]write-via-stream'}
    			FUNCTION_BLOCK RESULT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : Wasi.Cli.Types.ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'wasi:cli/stdout@0.3.0' '[future-drop-readable-1]write-via-stream'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'wasi:cli/stdout@0.3.0' '[future-drop-writable-1]write-via-stream'}
    			FUNCTION RESULT_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_FUTURE_WRITE`: a future's write is not imported yet.
       - `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}
