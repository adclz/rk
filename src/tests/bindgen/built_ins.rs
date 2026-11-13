// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The built-ins of streams and futures: one set per type, in
//! `Wasi.Streams` and `Wasi.Futures`, imported under the name of the first
//! function whose type has one, with the place the stream or future has in
//! that function's type. A stream of bytes is read into a STRING or an
//! ARRAY OF BYTE and written from a STRING; a stream of anything else is
//! made and dropped only; a future's value comes back as outputs, as a
//! function's would.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{checked, generated};
use crate::tests::utils::with_db;

/// Every stream of bytes takes one set, named after the first function
/// that has one, in another interface too; the futures of one type
/// likewise.
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
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-streams.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Streams
    		/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    		{extern 'wasi:example/pipe@0.3.0' '[stream-new-0]send'}
    		FUNCTION BYTES_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the bytes that come next into `data`, as many as it holds at most:
    		   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    		   go. */
    		{extern async 'wasi:example/pipe@0.3.0' '[stream-read-0]send'}
    		FUNCTION_BLOCK BYTES_READ
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
    		FUNCTION_BLOCK BYTES_READ_ARRAY
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
    		FUNCTION_BLOCK BYTES_WRITE
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
    		FUNCTION BYTES_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:example/pipe@0.3.0' '[stream-drop-writable-0]send'}
    		FUNCTION BYTES_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_CANCEL_READ` and `BYTES_CANCEL_WRITE`: a cancel blocks the
         scan. */

    // wasi-futures.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Futures
    		/* Makes a future of `result<_, error-code>`, and gives its reading end,
    		   then its writing end. */
    		{extern 'wasi:example/pipe@0.3.0' '[future-new-1]send'}
    		FUNCTION EXAMPLE_RESULT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/pipe@0.3.0' '[future-read-1]send'}
    		FUNCTION_BLOCK EXAMPLE_RESULT_READ
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
    		{extern 'wasi:example/pipe@0.3.0' '[future-drop-readable-1]send'}
    		FUNCTION EXAMPLE_RESULT_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/pipe@0.3.0' '[future-drop-writable-1]send'}
    		FUNCTION EXAMPLE_RESULT_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `EXAMPLE_RESULT_WRITE`: a future's write is not imported yet.
       - `EXAMPLE_RESULT_CANCEL_READ` and `EXAMPLE_RESULT_CANCEL_WRITE`: a
         cancel blocks the scan. */
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
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-streams.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Streams
    		/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    		{extern 'wasi:example/copy@0.3.0' '[stream-new-1][method]channel.transfer'}
    		FUNCTION BYTES_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the bytes that come next into `data`, as many as it holds at most:
    		   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    		   go. */
    		{extern async 'wasi:example/copy@0.3.0' '[stream-read-1][method]channel.transfer'}
    		FUNCTION_BLOCK BYTES_READ
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
    		FUNCTION_BLOCK BYTES_READ_ARRAY
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
    		FUNCTION_BLOCK BYTES_WRITE
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
    		FUNCTION BYTES_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:example/copy@0.3.0' '[stream-drop-writable-1][method]channel.transfer'}
    		FUNCTION BYTES_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_CANCEL_READ` and `BYTES_CANCEL_WRITE`: a cancel blocks the
         scan. */

    // wasi-futures.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Futures
    		/* Makes a future of `u32`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/copy@0.3.0' '[future-new-0][method]channel.transfer'}
    		FUNCTION U32_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/copy@0.3.0' '[future-read-0][method]channel.transfer'}
    		FUNCTION_BLOCK U32_READ
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
    		FUNCTION U32_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/copy@0.3.0' '[future-drop-writable-0][method]channel.transfer'}
    		FUNCTION U32_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_WRITE`: a future's write is not imported yet.
       - `U32_CANCEL_READ` and `U32_CANCEL_WRITE`: a cancel blocks the scan. */
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

    			{extern 'wasi:example/feeds@0.3.0' 'events'}
    			FUNCTION EVENTS : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/feeds@0.3.0' 'accept'}
    			FUNCTION ACCEPT : UDINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-streams.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Streams
    		/* Makes a stream of `u32`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]numbers'}
    		FUNCTION U32_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's reading end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]numbers'}
    		FUNCTION U32_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]numbers'}
    		FUNCTION U32_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a stream of `event`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]events'}
    		FUNCTION EVENT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's reading end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]events'}
    		FUNCTION EVENT_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]events'}
    		FUNCTION EVENT_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a stream of `own<connection>`, and gives its reading end, then its
    		   writing end. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-new-0]accept'}
    		FUNCTION CONNECTION_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's reading end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-readable-0]accept'}
    		FUNCTION CONNECTION_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:example/feeds@0.3.0' '[stream-drop-writable-0]accept'}
    		FUNCTION CONNECTION_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_READ` and `U32_WRITE`: a stream's read or write moves bytes only.
       - `U32_CANCEL_READ` and `U32_CANCEL_WRITE`: a cancel blocks the scan.
       - `EVENT_READ` and `EVENT_WRITE`: a stream's read or write moves bytes
         only.
       - `EVENT_CANCEL_READ` and `EVENT_CANCEL_WRITE`: a cancel blocks the
         scan.
       - `CONNECTION_READ` and `CONNECTION_WRITE`: a stream's read or write
         moves bytes only.
       - `CONNECTION_CANCEL_READ` and `CONNECTION_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}

/// A future's value comes back as a function's would: nothing, a scalar,
/// a flag and a side of a result, a flag and an option's value, a record's
/// fields; a string, or a result with both sides, is not read yet.
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
    			TYPE ERROR_CODE : USINT (
    				LOST := 0
    			); END_TYPE

    			{extern 'wasi:example/promises@0.3.0' 'ready'}
    			FUNCTION READY : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'count'}
    			FUNCTION COUNT : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'outcome'}
    			FUNCTION OUTCOME : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'maybe'}
    			FUNCTION MAYBE : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'where-is'}
    			FUNCTION WHERE_IS : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'name'}
    			FUNCTION NAME : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/promises@0.3.0' 'both'}
    			FUNCTION BOTH : UDINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-futures.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Futures
    		/* Makes a future of nothing, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]ready'}
    		FUNCTION UNIT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/promises@0.3.0' '[future-read-0]ready'}
    		FUNCTION_BLOCK UNIT_READ
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		VAR_OUTPUT
    			DONE : BOOL;
    		END_VAR
    		END_FUNCTION_BLOCK

    		/* Lets go of the future's reading end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]ready'}
    		FUNCTION UNIT_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]ready'}
    		FUNCTION UNIT_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `u32`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]count'}
    		FUNCTION U32_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/promises@0.3.0' '[future-read-0]count'}
    		FUNCTION_BLOCK U32_READ
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
    		FUNCTION U32_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]count'}
    		FUNCTION U32_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `result<_, error-code>`, and gives its reading end,
    		   then its writing end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]outcome'}
    		FUNCTION EXAMPLE_RESULT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/promises@0.3.0' '[future-read-0]outcome'}
    		FUNCTION_BLOCK EXAMPLE_RESULT_READ
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		VAR_OUTPUT
    			DONE : BOOL;
    			failed : BOOL;
    			error : Wasi.Example.Promises.ERROR_CODE;
    		END_VAR
    		END_FUNCTION_BLOCK

    		/* Lets go of the future's reading end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]outcome'}
    		FUNCTION EXAMPLE_RESULT_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]outcome'}
    		FUNCTION EXAMPLE_RESULT_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `option<s64>`, and gives its reading end, then its
    		   writing end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]maybe'}
    		FUNCTION OPTION_S64_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/promises@0.3.0' '[future-read-0]maybe'}
    		FUNCTION_BLOCK OPTION_S64_READ
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
    		FUNCTION OPTION_S64_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]maybe'}
    		FUNCTION OPTION_S64_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `point`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]where-is'}
    		FUNCTION POINT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:example/promises@0.3.0' '[future-read-0]where-is'}
    		FUNCTION_BLOCK POINT_READ
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
    		FUNCTION POINT_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]where-is'}
    		FUNCTION POINT_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `string`, and gives its reading end, then its writing
    		   end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]name'}
    		FUNCTION STRING_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's reading end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]name'}
    		FUNCTION STRING_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]name'}
    		FUNCTION STRING_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Makes a future of `result<u64, error-code>`, and gives its reading end,
    		   then its writing end. */
    		{extern 'wasi:example/promises@0.3.0' '[future-new-0]both'}
    		FUNCTION EXAMPLE_RESULT_U64_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's reading end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-readable-0]both'}
    		FUNCTION EXAMPLE_RESULT_U64_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:example/promises@0.3.0' '[future-drop-writable-0]both'}
    		FUNCTION EXAMPLE_RESULT_U64_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `UNIT_WRITE`: a future's write is not imported yet.
       - `UNIT_CANCEL_READ` and `UNIT_CANCEL_WRITE`: a cancel blocks the scan.
       - `U32_WRITE`: a future's write is not imported yet.
       - `U32_CANCEL_READ` and `U32_CANCEL_WRITE`: a cancel blocks the scan.
       - `EXAMPLE_RESULT_WRITE`: a future's write is not imported yet.
       - `EXAMPLE_RESULT_CANCEL_READ` and `EXAMPLE_RESULT_CANCEL_WRITE`: a
         cancel blocks the scan.
       - `OPTION_S64_WRITE`: a future's write is not imported yet.
       - `OPTION_S64_CANCEL_READ` and `OPTION_S64_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `POINT_WRITE`: a future's write is not imported yet.
       - `POINT_CANCEL_READ` and `POINT_CANCEL_WRITE`: a cancel blocks the
         scan.
       - `STRING_READ`: its value holds a string, which a read cannot give back
         yet.
       - `STRING_WRITE`: a future's write is not imported yet.
       - `STRING_CANCEL_READ` and `STRING_CANCEL_WRITE`: a cancel blocks the
         scan.
       - `EXAMPLE_RESULT_U64_READ`: its value holds a result with a value and
         an error, which a read cannot give back yet.
       - `EXAMPLE_RESULT_U64_WRITE`: a future's write is not imported yet.
       - `EXAMPLE_RESULT_U64_CANCEL_READ` and
         `EXAMPLE_RESULT_U64_CANCEL_WRITE`: a cancel blocks the scan. */
    ");
}

/// Two types whose sets would share a name stop the generator, which would
/// otherwise write two POUs of one name.
#[test]
fn two_sets_of_one_name_stop_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        interface left {
            record event { x: u32 }
            feed: func() -> stream<event>;
        }

        interface right {
            record event { y: u64 }
            feed: func() -> stream<event>;
        }

        world example {
            import left;
            import right;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"the streams of `wasi:example/left.event` and of `wasi:example/right.event` would both have built-ins named `EVENT`");
}

/// The sets of `wasi:cli`'s byte streams and futures, as Std's interfaces
/// have them: the file holds every built-in a stream of bytes has, and says
/// which it leaves out.
#[test]
fn std_s_streams_and_futures() {
    let bindings = generated(rk_bindgen::STD_WORLD);
    let built_ins: Vec<&str> = bindings
        .files
        .iter()
        .filter(|file| file.name == "wasi-streams.st" || file.name == "wasi-futures.st")
        .map(|file| file.text.as_str())
        .collect();
    assert_snapshot!(built_ins.join("\n"), @r"
    /* Generated by rk-bindgen from the WIT of wasi:cli@0.3.0. Do not edit:
       `cargo run -p rk-bindgen` writes it again. */
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Streams
    		/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    		{extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
    		FUNCTION BYTES_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the bytes that come next into `data`, as many as it holds at most:
    		   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    		   go. */
    		{extern async 'wasi:cli/stdout@0.3.0' '[stream-read-0]write-via-stream'}
    		FUNCTION_BLOCK BYTES_READ
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
    		FUNCTION_BLOCK BYTES_READ_ARRAY
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
    		FUNCTION_BLOCK BYTES_WRITE
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
    		FUNCTION BYTES_DROP_READABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the stream's writing end, `stream`. */
    		{extern 'wasi:cli/stdout@0.3.0' '[stream-drop-writable-0]write-via-stream'}
    		FUNCTION BYTES_DROP_WRITABLE
    		VAR_INPUT
    			stream : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_CANCEL_READ` and `BYTES_CANCEL_WRITE`: a cancel blocks the
         scan. */

    /* Generated by rk-bindgen from the WIT of wasi:cli@0.3.0. Do not edit:
       `cargo run -p rk-bindgen` writes it again. */
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Futures
    		/* Makes a future of `result<_, error-code>`, and gives its reading end,
    		   then its writing end. */
    		{extern 'wasi:cli/stdout@0.3.0' '[future-new-1]write-via-stream'}
    		FUNCTION CLI_RESULT_NEW
    		VAR_OUTPUT
    			readable : UDINT;
    			writable : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Reads the value the future holds, which comes back after `DONE`. */
    		{extern async 'wasi:cli/stdout@0.3.0' '[future-read-1]write-via-stream'}
    		FUNCTION_BLOCK CLI_RESULT_READ
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
    		FUNCTION CLI_RESULT_DROP_READABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION

    		/* Lets go of the future's writing end, `future`. */
    		{extern 'wasi:cli/stdout@0.3.0' '[future-drop-writable-1]write-via-stream'}
    		FUNCTION CLI_RESULT_DROP_WRITABLE
    		VAR_INPUT
    			future : UDINT;
    		END_VAR
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `CLI_RESULT_WRITE`: a future's write is not imported yet.
       - `CLI_RESULT_CANCEL_READ` and `CLI_RESULT_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}
