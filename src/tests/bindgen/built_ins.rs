// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The built-ins of streams and futures: one set per type in each
//! interface that takes or gives one, in its namespace, imported under the
//! name of its first function whose type has it, with the place the stream
//! or future has in that function's type, and named after the type and
//! what carries it: `BYTES_STREAM`, `RESULT_FUTURE`. A stream of bytes is
//! read into a STRING or an ARRAY OF BYTE and written from a STRING; one of
//! a resource's handles is read into an ARRAY OF UDINT; a stream of anything
//! else is made and dropped only; a future's value comes back as outputs, a
//! record in the STRUCT of one, as a function's would.

use std::pin::Pin;
use std::task::{Context, Poll};

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::StoreContextMut;
use wasmtime::component::{
    Destination, Linker, Resource, ResourceType, StreamProducer, StreamReader, StreamResult,
    VecBuffer,
};

use super::{assert_abi, built, checked, generated, generated_std, hosted, summary};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// A set per type in each interface: the two functions of one interface
/// that take or give a stream of bytes share one, named after the first;
/// another interface has its own, named after its own function.
#[rstest]
fn one_set_per_type_across_functions_and_interfaces(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

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
    // acme-example-pipe.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Pipe
    			TYPE ERROR_CODE : USINT (
    				CLOSED := 0,
    				FULL := 1
    			); END_TYPE

    			{extern 'acme:example/pipe@0.3.0' 'send'}
    			FUNCTION SEND : UDINT
    			VAR_INPUT
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'acme:example/pipe@0.3.0' '[stream-new-0]send'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the bytes that come next into `data`, as many as it holds at most:
    			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
    			   go. */
    			{extern async 'acme:example/pipe@0.3.0' '[stream-read-0]send'}
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
    			{extern async 'acme:example/pipe@0.3.0' '[stream-read-0]send'}
    			FUNCTION_BLOCK BYTES_STREAM_READ_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..8191] OF BYTE;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/pipe@0.3.0' '[stream-write-0]send'}
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

    			/* Writes the `count` bytes from the one `data` points at, as many as the
    			   reader takes: `written` of them, after `DONE`. The host reads them where
    			   they are while the write runs, so they stay there, unchanged, until
    			   `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/pipe@0.3.0' '[stream-write-0]send'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    				data : REF_TO BYTE;
    				count : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/pipe@0.3.0' '[stream-drop-readable-0]send'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'acme:example/pipe@0.3.0' '[stream-drop-writable-0]send'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* The bytes `IN` holds, which `BYTES_STREAM_WRITER` counts. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_LEN : UDINT
    			VAR_INPUT
    				IN : STRING[4096];
    			END_VAR
    				{wasm 'str.byte_len' (params IN) (result BYTES_STREAM_TEXT_LEN)}
    			END_FUNCTION

    			/* `IN` without its first `L` bytes: what `BYTES_STREAM_WRITER` has left to
    			   write. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_AFTER : STRING[4096]
    			VAR_INPUT
    				IN : STRING[4096];
    				L : UDINT;
    			END_VAR
    			VAR
    				P : UDINT := 1;
    			END_VAR
    				{wasm 'str.byte_delete' (params IN L P) (result BYTES_STREAM_TEXT_AFTER)}
    			END_FUNCTION

    			/* Writes text to a stream of bytes it makes at its first call, for the
    			   function `readable` is handed to. */
    			FUNCTION_BLOCK BYTES_STREAM_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* The stream's reading end, to hand to the function that reads it. */
    				readable : UDINT;
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, or a request comes after
    				   `CLOSE`: the stream does not open again. */
    				ERROR : BOOL;
    			END_VAR
    			VAR
    				writable : UDINT;
    				requested : BOOL;
    				closing : BOOL;
    				shut : BOOL;
    				closed : BOOL;
    				rest : STRING[4096];
    				write : BYTES_STREAM_WRITE;
    			END_VAR
    			VAR_TEMP
    				again : BOOL;
    			END_VAR
    				IF writable = 0 AND NOT closed THEN
    					BYTES_STREAM_NEW(readable => readable, writable => writable);
    				END_IF;
    				DONE := FALSE;
    				// An edge while BUSY is ignored.
    				IF REQ AND NOT requested AND NOT BUSY THEN
    					ERROR := closed;
    					IF NOT closed THEN
    						// A write of no bytes asks the reader whether it is ready,
    						// which it may answer late: an empty text is done here.
    						IF BYTES_STREAM_TEXT_LEN(IN) = 0 THEN
    							DONE := TRUE;
    						ELSE
    							rest := IN;
    							BUSY := TRUE;
    						END_IF;
    					END_IF;
    				END_IF;
    				requested := REQ;
    				IF BUSY THEN
    					// Each pass after the first hands over a byte at least, so the
    					// loop ends.
    					REPEAT
    						again := FALSE;
    						// Starts the write of the rest, or checks the one running.
    						write(stream := writable, data := rest);
    						IF write.DONE THEN
    							IF write.closed THEN
    								// The reader left: the writing end goes too.
    								BYTES_STREAM_DROP_WRITABLE(stream := writable);
    								writable := 0;
    								closed := TRUE;
    								BUSY := FALSE;
    								ERROR := TRUE;
    							ELSIF write.written < BYTES_STREAM_TEXT_LEN(rest) THEN
    								// The reader took a part: the rest goes next, at once while
    								// it takes some.
    								rest := BYTES_STREAM_TEXT_AFTER(rest, write.written);
    								again := write.written > 0;
    							ELSE
    								BUSY := FALSE;
    								DONE := TRUE;
    							END_IF;
    						END_IF;
    					UNTIL NOT again
    					END_REPEAT;
    				END_IF;
    				// The writing end goes once no write is under way.
    				IF CLOSE AND NOT closing THEN
    					shut := TRUE;
    				END_IF;
    				closing := CLOSE;
    				IF shut AND NOT BUSY THEN
    					IF writable <> 0 THEN
    						BYTES_STREAM_DROP_WRITABLE(stream := writable);
    						writable := 0;
    					END_IF;
    					closed := TRUE;
    					shut := FALSE;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/pipe@0.3.0' '[future-read-1]send'}
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
    			{extern 'acme:example/pipe@0.3.0' '[future-drop-readable-1]send'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK RESULT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						failed := call.failed;
    						error := call.error;
    						RESULT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/pipe@0.3.0' 'receive'}
    			FUNCTION RECEIVE
    			VAR_OUTPUT
    				item0 : UDINT;
    				item1 : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Writes text to `send` through a stream of bytes it opens at the first
    			   request. */
    			FUNCTION_BLOCK SEND_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`; the first opens the stream and calls `send`
    				   with it. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, a request comes after `CLOSE`,
    				   or the `result<_, error-code>` from `send` is an error, whose
    				   `error-code` is in `ERRORID`. */
    				ERROR : BOOL;
    				/* The `error-code` of the `result<_, error-code>` from `send`, when it is
    				   an error, which turns `ERROR` TRUE. */
    				ERRORID : ERROR_CODE;
    				/* TRUE once the `future<result<_, error-code>>` from `send` has its value. */
    				ENDED : BOOL;
    			END_VAR
    			VAR
    				open : BOOL;
    				stream : BYTES_STREAM_WRITER;
    				future : UDINT;
    				answer : RESULT_FUTURE_GET;
    			END_VAR
    				// The stream, made at the first request and handed to the function.
    				IF REQ AND NOT open THEN
    					stream();
    					future := Acme.Example.Pipe.SEND(data := stream.readable);
    					open := TRUE;
    				END_IF;
    				IF open THEN
    					stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    					BUSY := stream.BUSY;
    					DONE := stream.DONE;
    					// The answer, read once it comes, and kept.
    					answer(future := future);
    					IF answer.DONE THEN
    						ERRORID := answer.error;
    						ENDED := TRUE;
    					END_IF;
    					ERROR := stream.ERROR OR answer.failed;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Reads what `receive` gives through a stream of bytes, a request at a
    			   time. */
    			FUNCTION_BLOCK RECEIVE_READER
    			VAR_INPUT
    				/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    				   `receive`. An edge while `BUSY` is ignored, and one after the stream
    				   ended is `DONE` at once, `OUT` empty. */
    				REQ : BOOL;
    				/* A rising edge lets go of the stream, once no read is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* TRUE while a read waits for bytes. */
    				BUSY : BOOL;
    				/* TRUE in the call where a read has given what came, in `OUT`. */
    				DONE : BOOL;
    				/* What the last read gave: empty once the stream has ended, nothing more
    				   coming. */
    				OUT : STRING[4096];
    				/* TRUE once the `result<_, error-code>` from `receive` is an error, whose
    				   `error-code` is in `ERRORID`. */
    				ERROR : BOOL;
    				/* The `error-code` of the `result<_, error-code>` from `receive`, when it
    				   is an error, which turns `ERROR` TRUE. */
    				ERRORID : ERROR_CODE;
    				/* TRUE once the stream has ended and the `future<result<_, error-code>>`
    				   from `receive` has its value. */
    				ENDED : BOOL;
    			END_VAR
    			VAR
    				open : BOOL;
    				requested : BOOL;
    				closing : BOOL;
    				shut : BOOL;
    				stream : UDINT;
    				future : UDINT;
    				read : BYTES_STREAM_READ;
    				answer : RESULT_FUTURE_GET;
    				came : BOOL;
    			END_VAR
    				DONE := FALSE;
    				// The stream, given by the function at the first request.
    				IF REQ AND NOT open THEN
    					Acme.Example.Pipe.RECEIVE(item0 => stream, item1 => future);
    					open := TRUE;
    				END_IF;
    				IF open THEN
    					// An edge while BUSY is ignored.
    					IF REQ AND NOT requested AND NOT BUSY THEN
    						IF stream <> 0 THEN
    							BUSY := TRUE;
    						ELSE
    							// The stream has ended: nothing more comes.
    							OUT := '';
    							DONE := TRUE;
    						END_IF;
    					END_IF;
    					requested := REQ;
    					IF BUSY THEN
    						// Starts the read, or checks the one running.
    						read(stream := stream);
    						IF read.DONE THEN
    							OUT := read.data;
    							BUSY := FALSE;
    							DONE := TRUE;
    							IF read.closed THEN
    								// The writer left: the reading end goes too.
    								BYTES_STREAM_DROP_READABLE(stream := stream);
    								stream := 0;
    							END_IF;
    						END_IF;
    					END_IF;
    					// The reading end goes on CLOSE, once no read is under way.
    					IF CLOSE AND NOT closing THEN
    						shut := TRUE;
    					END_IF;
    					closing := CLOSE;
    					IF shut AND NOT BUSY THEN
    						IF stream <> 0 THEN
    							BYTES_STREAM_DROP_READABLE(stream := stream);
    							stream := 0;
    						END_IF;
    						shut := FALSE;
    					END_IF;
    					// The answer, read once it comes, and kept.
    					answer(future := future);
    					IF answer.DONE THEN
    						ERRORID := answer.error;
    						came := TRUE;
    					END_IF;
    					ENDED := stream = 0 AND came;
    					ERROR := answer.failed;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */

    // acme-example-other.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Other
    			{extern 'acme:example/other@0.3.0' 'relay'}
    			FUNCTION RELAY : UDINT
    			VAR_INPUT
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'acme:example/other@0.3.0' '[stream-new-0]relay'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/other@0.3.0' '[stream-write-0]relay'}
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

    			/* Writes the `count` bytes from the one `data` points at, as many as the
    			   reader takes: `written` of them, after `DONE`. The host reads them where
    			   they are while the write runs, so they stay there, unchanged, until
    			   `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/other@0.3.0' '[stream-write-0]relay'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    				data : REF_TO BYTE;
    				count : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/other@0.3.0' '[stream-drop-readable-0]relay'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'acme:example/other@0.3.0' '[stream-drop-writable-0]relay'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* The bytes `IN` holds, which `BYTES_STREAM_WRITER` counts. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_LEN : UDINT
    			VAR_INPUT
    				IN : STRING[4096];
    			END_VAR
    				{wasm 'str.byte_len' (params IN) (result BYTES_STREAM_TEXT_LEN)}
    			END_FUNCTION

    			/* `IN` without its first `L` bytes: what `BYTES_STREAM_WRITER` has left to
    			   write. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_AFTER : STRING[4096]
    			VAR_INPUT
    				IN : STRING[4096];
    				L : UDINT;
    			END_VAR
    			VAR
    				P : UDINT := 1;
    			END_VAR
    				{wasm 'str.byte_delete' (params IN L P) (result BYTES_STREAM_TEXT_AFTER)}
    			END_FUNCTION

    			/* Writes text to a stream of bytes it makes at its first call, for the
    			   function `readable` is handed to. */
    			FUNCTION_BLOCK BYTES_STREAM_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* The stream's reading end, to hand to the function that reads it. */
    				readable : UDINT;
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, or a request comes after
    				   `CLOSE`: the stream does not open again. */
    				ERROR : BOOL;
    			END_VAR
    			VAR
    				writable : UDINT;
    				requested : BOOL;
    				closing : BOOL;
    				shut : BOOL;
    				closed : BOOL;
    				rest : STRING[4096];
    				write : BYTES_STREAM_WRITE;
    			END_VAR
    			VAR_TEMP
    				again : BOOL;
    			END_VAR
    				IF writable = 0 AND NOT closed THEN
    					BYTES_STREAM_NEW(readable => readable, writable => writable);
    				END_IF;
    				DONE := FALSE;
    				// An edge while BUSY is ignored.
    				IF REQ AND NOT requested AND NOT BUSY THEN
    					ERROR := closed;
    					IF NOT closed THEN
    						// A write of no bytes asks the reader whether it is ready,
    						// which it may answer late: an empty text is done here.
    						IF BYTES_STREAM_TEXT_LEN(IN) = 0 THEN
    							DONE := TRUE;
    						ELSE
    							rest := IN;
    							BUSY := TRUE;
    						END_IF;
    					END_IF;
    				END_IF;
    				requested := REQ;
    				IF BUSY THEN
    					// Each pass after the first hands over a byte at least, so the
    					// loop ends.
    					REPEAT
    						again := FALSE;
    						// Starts the write of the rest, or checks the one running.
    						write(stream := writable, data := rest);
    						IF write.DONE THEN
    							IF write.closed THEN
    								// The reader left: the writing end goes too.
    								BYTES_STREAM_DROP_WRITABLE(stream := writable);
    								writable := 0;
    								closed := TRUE;
    								BUSY := FALSE;
    								ERROR := TRUE;
    							ELSIF write.written < BYTES_STREAM_TEXT_LEN(rest) THEN
    								// The reader took a part: the rest goes next, at once while
    								// it takes some.
    								rest := BYTES_STREAM_TEXT_AFTER(rest, write.written);
    								again := write.written > 0;
    							ELSE
    								BUSY := FALSE;
    								DONE := TRUE;
    							END_IF;
    						END_IF;
    					UNTIL NOT again
    					END_REPEAT;
    				END_IF;
    				// The writing end goes once no write is under way.
    				IF CLOSE AND NOT closing THEN
    					shut := TRUE;
    				END_IF;
    				closing := CLOSE;
    				IF shut AND NOT BUSY THEN
    					IF writable <> 0 THEN
    						BYTES_STREAM_DROP_WRITABLE(stream := writable);
    						writable := 0;
    					END_IF;
    					closed := TRUE;
    					shut := FALSE;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/other@0.3.0' '[future-read-1]relay'}
    			FUNCTION_BLOCK RESULT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : Acme.Example.Pipe.ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/other@0.3.0' '[future-drop-readable-1]relay'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK RESULT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : Acme.Example.Pipe.ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						failed := call.failed;
    						error := call.error;
    						RESULT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Writes text to `relay` through a stream of bytes it opens at the first
    			   request. */
    			FUNCTION_BLOCK RELAY_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`; the first opens the stream and calls `relay`
    				   with it. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, a request comes after `CLOSE`,
    				   or the `result<_, error-code>` from `relay` is an error, whose
    				   `error-code` is in `ERRORID`. */
    				ERROR : BOOL;
    				/* The `error-code` of the `result<_, error-code>` from `relay`, when it is
    				   an error, which turns `ERROR` TRUE. */
    				ERRORID : Acme.Example.Pipe.ERROR_CODE;
    				/* TRUE once the `future<result<_, error-code>>` from `relay` has its
    				   value. */
    				ENDED : BOOL;
    			END_VAR
    			VAR
    				open : BOOL;
    				stream : BYTES_STREAM_WRITER;
    				future : UDINT;
    				answer : RESULT_FUTURE_GET;
    			END_VAR
    				// The stream, made at the first request and handed to the function.
    				IF REQ AND NOT open THEN
    					stream();
    					future := Acme.Example.Other.RELAY(data := stream.readable);
    					open := TRUE;
    				END_IF;
    				IF open THEN
    					stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    					BUSY := stream.BUSY;
    					DONE := stream.DONE;
    					// The answer, read once it comes, and kept.
    					answer(future := future);
    					IF answer.DONE THEN
    						ERRORID := answer.error;
    						ENDED := TRUE;
    					END_IF;
    					ERROR := stream.ERROR OR answer.failed;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan.
       - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
    ");
}

/// The place in a function's type counts streams and futures together, in
/// the order WIT walks the type: the second is `-1`. A method's built-ins
/// are named after the method.
#[rstest]
fn the_place_counts_streams_and_futures_together(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

        interface copy {
            resource channel {
                transfer: func(from-here: future<u32>, data: stream<u8>) -> bool;
            }
        }

        world example {
            import copy;
        }
    "#), @r"
    // acme-example-copy.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Copy
    			TYPE CHANNEL : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `channel` the host gave. */
    			{extern 'acme:example/copy@0.3.0' '[resource-drop]channel'}
    			FUNCTION CHANNEL_DROP
    			VAR_INPUT
    				handle : CHANNEL;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/copy@0.3.0' '[method]channel.transfer'}
    			FUNCTION CHANNEL_TRANSFER : BOOL
    			VAR_INPUT
    				self : CHANNEL;
    				from_here : UDINT;
    				data : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a future of `u32`, and gives its reading end, then its writing
    			   end. */
    			{extern 'acme:example/copy@0.3.0' '[future-new-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/copy@0.3.0' '[future-drop-readable-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the future's writing end, `future`. */
    			{extern 'acme:example/copy@0.3.0' '[future-drop-writable-0][method]channel.transfer'}
    			FUNCTION U32_FUTURE_DROP_WRITABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
    			{extern 'acme:example/copy@0.3.0' '[stream-new-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Writes the bytes of `data`, as many as the reader takes: `written` of
    			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/copy@0.3.0' '[stream-write-1][method]channel.transfer'}
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

    			/* Writes the `count` bytes from the one `data` points at, as many as the
    			   reader takes: `written` of them, after `DONE`. The host reads them where
    			   they are while the write runs, so they stay there, unchanged, until
    			   `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'acme:example/copy@0.3.0' '[stream-write-1][method]channel.transfer'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    				data : REF_TO BYTE;
    				count : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				written : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/copy@0.3.0' '[stream-drop-readable-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'acme:example/copy@0.3.0' '[stream-drop-writable-1][method]channel.transfer'}
    			FUNCTION BYTES_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* The bytes `IN` holds, which `BYTES_STREAM_WRITER` counts. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_LEN : UDINT
    			VAR_INPUT
    				IN : STRING[4096];
    			END_VAR
    				{wasm 'str.byte_len' (params IN) (result BYTES_STREAM_TEXT_LEN)}
    			END_FUNCTION

    			/* `IN` without its first `L` bytes: what `BYTES_STREAM_WRITER` has left to
    			   write. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_AFTER : STRING[4096]
    			VAR_INPUT
    				IN : STRING[4096];
    				L : UDINT;
    			END_VAR
    			VAR
    				P : UDINT := 1;
    			END_VAR
    				{wasm 'str.byte_delete' (params IN L P) (result BYTES_STREAM_TEXT_AFTER)}
    			END_FUNCTION

    			/* Writes text to a stream of bytes it makes at its first call, for the
    			   function `readable` is handed to. */
    			FUNCTION_BLOCK BYTES_STREAM_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* The stream's reading end, to hand to the function that reads it. */
    				readable : UDINT;
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, or a request comes after
    				   `CLOSE`: the stream does not open again. */
    				ERROR : BOOL;
    			END_VAR
    			VAR
    				writable : UDINT;
    				requested : BOOL;
    				closing : BOOL;
    				shut : BOOL;
    				closed : BOOL;
    				rest : STRING[4096];
    				write : BYTES_STREAM_WRITE;
    			END_VAR
    			VAR_TEMP
    				again : BOOL;
    			END_VAR
    				IF writable = 0 AND NOT closed THEN
    					BYTES_STREAM_NEW(readable => readable, writable => writable);
    				END_IF;
    				DONE := FALSE;
    				// An edge while BUSY is ignored.
    				IF REQ AND NOT requested AND NOT BUSY THEN
    					ERROR := closed;
    					IF NOT closed THEN
    						// A write of no bytes asks the reader whether it is ready,
    						// which it may answer late: an empty text is done here.
    						IF BYTES_STREAM_TEXT_LEN(IN) = 0 THEN
    							DONE := TRUE;
    						ELSE
    							rest := IN;
    							BUSY := TRUE;
    						END_IF;
    					END_IF;
    				END_IF;
    				requested := REQ;
    				IF BUSY THEN
    					// Each pass after the first hands over a byte at least, so the
    					// loop ends.
    					REPEAT
    						again := FALSE;
    						// Starts the write of the rest, or checks the one running.
    						write(stream := writable, data := rest);
    						IF write.DONE THEN
    							IF write.closed THEN
    								// The reader left: the writing end goes too.
    								BYTES_STREAM_DROP_WRITABLE(stream := writable);
    								writable := 0;
    								closed := TRUE;
    								BUSY := FALSE;
    								ERROR := TRUE;
    							ELSIF write.written < BYTES_STREAM_TEXT_LEN(rest) THEN
    								// The reader took a part: the rest goes next, at once while
    								// it takes some.
    								rest := BYTES_STREAM_TEXT_AFTER(rest, write.written);
    								again := write.written > 0;
    							ELSE
    								BUSY := FALSE;
    								DONE := TRUE;
    							END_IF;
    						END_IF;
    					UNTIL NOT again
    					END_REPEAT;
    				END_IF;
    				// The writing end goes once no write is under way.
    				IF CLOSE AND NOT closing THEN
    					shut := TRUE;
    				END_IF;
    				closing := CLOSE;
    				IF shut AND NOT BUSY THEN
    					IF writable <> 0 THEN
    						BYTES_STREAM_DROP_WRITABLE(stream := writable);
    						writable := 0;
    					END_IF;
    					closed := TRUE;
    					shut := FALSE;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_FUTURE_WRITE`: a future's write is not imported yet.
       - `U32_FUTURE_CANCEL_WRITE`: a cancel blocks the scan.
       - `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan. */
    ");
}

/// A stream of the handles of a resource, `connection`, as a listener's
/// connections come in.
const FEEDS: &str = r#"
    package acme:example@0.3.0;

    interface feeds {
        resource connection;
        accept: func() -> stream<connection>;
    }

    world example {
        import feeds;
    }
"#;

/// A stream of a resource's handles is read into an array of the
/// resource's type, a UDINT, a handle an element, each the program's own to
/// drop: what a listener's connections come in. Nothing takes such a stream
/// from the program, which a write would feed.
#[rstest]
fn a_stream_of_handles_is_read_into_an_array(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, FEEDS), @r"
    // acme-example-feeds.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Feeds
    			TYPE CONNECTION : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `connection` the host gave. */
    			{extern 'acme:example/feeds@0.3.0' '[resource-drop]connection'}
    			FUNCTION CONNECTION_DROP
    			VAR_INPUT
    				handle : CONNECTION;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/feeds@0.3.0' 'accept'}
    			FUNCTION ACCEPT : UDINT
    			END_FUNCTION

    			/* Reads the handles that come next into `data`, as many as it holds at
    			   most: `read` of them, after `DONE`, each the program's own to drop.
    			   `closed` is TRUE once the writer has let go. */
    			{extern async 'acme:example/feeds@0.3.0' '[stream-read-0]accept'}
    			FUNCTION_BLOCK CONNECTION_STREAM_READ
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				data : ARRAY[0..15] OF CONNECTION;
    				read : UDINT;
    				closed : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-drop-readable-0]accept'}
    			FUNCTION CONNECTION_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `CONNECTION_STREAM_CANCEL_READ`: a cancel blocks the scan. */
    ");
}

/// A read takes as many handles as its array holds, and no more: a host
/// that gives three connections at once fills a block whose array holds two
/// with the first two, each a handle the program drops. 1 when it does, and
/// otherwise the number of the check that failed, negative.
#[rstest]
fn a_read_takes_as_many_handles_as_its_array_holds(mut with_db: RootDatabase) {
    let program = r#"
        {extern async 'acme:example/feeds@0.3.0' '[stream-read-0]accept'}
        FUNCTION_BLOCK TWO_AT_ONCE
        VAR_INPUT stream : UDINT; END_VAR
        VAR_OUTPUT DONE : BOOL; data : ARRAY[0..1] OF UDINT; read : UDINT; closed : BOOL; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            connections : UDINT;
            two : TWO_AT_ONCE;
        END_VAR
            CASE step OF
            0:
                connections := Acme.Example.Feeds.ACCEPT();
                step := 1;
            1:
                two(stream := connections);
                IF two.DONE THEN
                    step := 9;
                    IF two.read <> 2 OR two.closed THEN output := -1; RETURN; END_IF;
                    Acme.Example.Feeds.CONNECTION_DROP(handle := two.data[0]);
                    Acme.Example.Feeds.CONNECTION_DROP(handle := two.data[1]);
                    output := 1;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#;
    let mut plc = hosted(&mut with_db, FEEDS, program, feeds_host);
    let mut output = 0;
    for _ in 0..20 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}

/// The host's side of [`FEEDS`]: `accept` gives a stream of three
/// connections, all at once.
fn feeds_host(linker: &mut Linker<Host>) {
    let mut feeds = linker
        .instance("acme:example/feeds@0.3.0")
        .expect("the interface");
    feeds
        .resource("connection", ResourceType::host::<Connection>(), |_, _| {
            Ok(())
        })
        .expect("the resource");
    feeds
        .func_wrap("accept", |mut store: StoreContextMut<'_, Host>, (): ()| {
            Ok((StreamReader::new(&mut store, Connections(3))?,))
        })
        .expect("a function of the host");
}

/// A connection the host gives.
struct Connection;

/// A stream of connections that gives the first `.0`, numbered from 1, at
/// its first read, and nothing after.
struct Connections(u32);

impl<D> StreamProducer<D> for Connections {
    type Item = Resource<Connection>;
    type Buffer = VecBuffer<Resource<Connection>>;

    fn poll_produce<'a>(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        mut store: StoreContextMut<'a, D>,
        mut destination: Destination<'a, Self::Item, Self::Buffer>,
        finish: bool,
    ) -> Poll<wasmtime::Result<StreamResult>> {
        // A read of nothing asks whether the stream is ready.
        if destination.remaining(&mut store) == Some(0) {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        // Given out: nothing comes after, until the read is called off.
        if self.0 == 0 {
            return match finish {
                true => Poll::Ready(Ok(StreamResult::Cancelled)),
                false => Poll::Pending,
            };
        }
        let given: Vec<Resource<Connection>> = (1..=self.0).map(Resource::new_own).collect();
        self.0 = 0;
        destination.set_buffer(given.into());
        Poll::Ready(Ok(StreamResult::Completed))
    }
}

/// A stream of anything but bytes or handles is made and dropped, not read
/// or written yet: of a number, of a record, given to the program and
/// taken from it.
#[rstest]
fn a_stream_of_anything_else_is_made_and_dropped(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

        interface feeds {
            record event { code: u32 }
            numbers: func() -> stream<u32>;
            events: func() -> stream<event>;
            count: func(numbers: stream<u32>);
            log: func(events: stream<event>);
        }

        world example {
            import feeds;
        }
    "#), @r"
    // acme-example-feeds.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Feeds
    			TYPE EVENT : STRUCT
    				code : UDINT;
    			END_STRUCT END_TYPE

    			{extern 'acme:example/feeds@0.3.0' 'numbers'}
    			FUNCTION NUMBERS : UDINT
    			END_FUNCTION

    			/* Makes a stream of `u32`, and gives its reading end, then its writing
    			   end. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-new-0]numbers'}
    			FUNCTION U32_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-drop-readable-0]numbers'}
    			FUNCTION U32_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-drop-writable-0]numbers'}
    			FUNCTION U32_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/feeds@0.3.0' 'events'}
    			FUNCTION EVENTS : UDINT
    			END_FUNCTION

    			/* Makes a stream of `event`, and gives its reading end, then its writing
    			   end. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-new-0]events'}
    			FUNCTION EVENT_STREAM_NEW
    			VAR_OUTPUT
    				readable : UDINT;
    				writable : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's reading end, `stream`. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-drop-readable-0]events'}
    			FUNCTION EVENT_STREAM_DROP_READABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of the stream's writing end, `stream`. */
    			{extern 'acme:example/feeds@0.3.0' '[stream-drop-writable-0]events'}
    			FUNCTION EVENT_STREAM_DROP_WRITABLE
    			VAR_INPUT
    				stream : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/feeds@0.3.0' 'count'}
    			FUNCTION COUNT
    			VAR_INPUT
    				numbers : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/feeds@0.3.0' 'log'}
    			FUNCTION LOG
    			VAR_INPUT
    				events : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `U32_STREAM_READ`: a stream's read moves bytes or handles only.
       - `U32_STREAM_WRITE`: a stream's write moves bytes only.
       - `U32_STREAM_CANCEL_READ` and `U32_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan.
       - `EVENT_STREAM_READ`: a stream's read moves bytes or handles only.
       - `EVENT_STREAM_WRITE`: a stream's write moves bytes only.
       - `EVENT_STREAM_CANCEL_READ` and `EVENT_STREAM_CANCEL_WRITE`: a cancel
         blocks the scan. */
    ");
}

/// A write left out says why: where a function of the interface takes a
/// stream of handles or a future from the program, it is what rk does not
/// write yet. Where none does, the program writes none, and no note says
/// so.
#[rstest]
fn a_write_left_out_says_why(mut with_db: RootDatabase) {
    let bindings = generated(
        r#"
        package acme:example@0.3.0;

        interface hand {
            resource connection;
            accept: func() -> stream<connection>;
            hand-over: func(connections: stream<connection>);
            later: func() -> future<u32>;
            give: func(value: future<u32>);
        }

        interface keep {
            resource connection;
            accept: func() -> stream<connection>;
            later: func() -> future<u32>;
        }

        world example {
            import hand;
            import keep;
        }
    "#,
    );
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    assert_snapshot!(summary(&bindings.files), @r"
    acme-example-hand.st: CONNECTION, CONNECTION_DROP, ACCEPT, CONNECTION_STREAM_NEW, CONNECTION_STREAM_READ, CONNECTION_STREAM_DROP_READABLE, CONNECTION_STREAM_DROP_WRITABLE, HAND_OVER, LATER, U32_FUTURE_NEW, U32_FUTURE_READ, U32_FUTURE_DROP_READABLE, U32_FUTURE_DROP_WRITABLE, U32_FUTURE_GET, GIVE
        left out `CONNECTION_STREAM_WRITE`: a stream's write moves bytes only
        left out `CONNECTION_STREAM_CANCEL_READ` and `CONNECTION_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `U32_FUTURE_WRITE`: a future's write is not imported yet
        left out `U32_FUTURE_CANCEL_READ` and `U32_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    acme-example-keep.st: CONNECTION, CONNECTION_DROP, ACCEPT, CONNECTION_STREAM_READ, CONNECTION_STREAM_DROP_READABLE, LATER, U32_FUTURE_READ, U32_FUTURE_DROP_READABLE, U32_FUTURE_GET
        left out `CONNECTION_STREAM_CANCEL_READ`: a cancel blocks the scan
        left out `U32_FUTURE_CANCEL_READ`: a cancel blocks the scan
    ");
}

/// A function that takes one stream of bytes from the program has a writer
/// of its own, `<NAME>_WRITER`: its other inputs are the block's, a name of
/// the block's own taking a `_`. A `func` that answers with a future or
/// nothing is called: a future no read gives back is let go of unread, and
/// one that is read gives its value as outputs. The call of an `async func`
/// is started and followed to its end, its result kept as outputs. `done`,
/// named as the writer's own `DONE`, is called by its namespace's path. A
/// function that takes two streams, or a stream and a future, a `func` that
/// answers with anything else, and an `async func` whose result carries a
/// future, have none.
#[rstest]
fn a_function_that_takes_a_stream_has_a_writer(mut with_db: RootDatabase) {
    let bindings = generated(
        r#"
        package acme:example@0.3.0;

        interface pipe {
            resource channel {
                send: func(data: stream<u8>, req: u32) -> future<result<_, string>>;
            }
            post: func(data: stream<u8>) -> future<u32>;
            sink: func(data: stream<u8>);
            done: func(data: stream<u8>);
            copy: func(a: stream<u8>, b: stream<u8>);
            relay: func(data: stream<u8>, later: future<u32>);
            count: func(data: stream<u8>) -> u32;
            upload: async func(data: stream<u8>, done: u32) -> u32;
            push: async func(data: stream<u8>);
            defer: async func(data: stream<u8>) -> future<u32>;
        }

        world example {
            import pipe;
        }
    "#,
    );
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    // The writers of the functions, without the stream's own.
    let writers: String = bindings
        .files
        .iter()
        .flat_map(|file| file.text.split("\n\n"))
        .filter(|block| block.contains("_WRITER\n") && !block.contains("BYTES_STREAM_WRITER\n"))
        .filter_map(|block| block.split_inclusive("END_FUNCTION_BLOCK").next())
        .map(|block| format!("{}\n\n", block.trim_matches('\n')))
        .collect();
    assert_snapshot!(writers, @r"
    /* Writes text to `channel.send` through a stream of bytes it opens at the
       first request. */
    FUNCTION_BLOCK CHANNEL_SEND_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and calls
    	   `channel.send` with it. An edge while `BUSY` is ignored. `req_` is a
    	   parameter of `channel.send`. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    	/* `self: borrow<channel>` of `channel.send`, passed when the stream opens. */
    	self : CHANNEL;
    	/* `req: u32` of `channel.send`, passed when the stream opens. Not `REQ`,
    	   the writer's own. */
    	req_ : UDINT;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    END_VAR
    	// The stream, made at the first request and handed to the function.
    	IF REQ AND NOT open THEN
    		stream();
    		RESULT_FUTURE_DROP_READABLE(future := Acme.Example.Pipe.CHANNEL_SEND(data := stream.readable, self := self, req := req_));
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Writes text to `post` through a stream of bytes it opens at the first
       request. */
    FUNCTION_BLOCK POST_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and calls `post`
    	   with it. An edge while `BUSY` is ignored. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    	/* TRUE once the `future<u32>` from `post` has its value. */
    	ENDED : BOOL;
    	/* The `u32` from `post`. */
    	value : UDINT;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    	future : UDINT;
    	answer : U32_FUTURE_GET;
    END_VAR
    	// The stream, made at the first request and handed to the function.
    	IF REQ AND NOT open THEN
    		stream();
    		future := Acme.Example.Pipe.POST(data := stream.readable);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		// The answer, read once it comes, and kept.
    		answer(future := future);
    		IF answer.DONE THEN
    			value := answer.value;
    			ENDED := TRUE;
    		END_IF;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Writes text to `sink` through a stream of bytes it opens at the first
       request. */
    FUNCTION_BLOCK SINK_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and calls `sink`
    	   with it. An edge while `BUSY` is ignored. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    END_VAR
    	// The stream, made at the first request and handed to the function.
    	IF REQ AND NOT open THEN
    		stream();
    		Acme.Example.Pipe.SINK(data := stream.readable);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Writes text to `done` through a stream of bytes it opens at the first
       request. */
    FUNCTION_BLOCK DONE_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and calls `done`
    	   with it. An edge while `BUSY` is ignored. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    END_VAR
    	// The stream, made at the first request and handed to the function.
    	IF REQ AND NOT open THEN
    		stream();
    		Acme.Example.Pipe.DONE(data := stream.readable);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Writes text to `upload` through a stream of bytes it opens at the first
       request. */
    FUNCTION_BLOCK UPLOAD_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and starts
    	   `upload` with it. An edge while `BUSY` is ignored. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    	/* `done: u32` of `upload`, passed when the stream opens. Not `DONE`, the
    	   writer's own. */
    	done_ : UDINT;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. `done_` is a parameter of `upload`. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    	/* TRUE once `upload` has returned. */
    	ENDED : BOOL;
    	/* The `u32` from `upload`. */
    	value : UDINT;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    	call : Acme.Example.Pipe.UPLOAD;
    END_VAR
    	// The stream, made at the first request.
    	IF REQ AND NOT open THEN
    		stream();
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// The call, started with the stream's reading end at the first
    		// request and followed to its end, once.
    		IF NOT ENDED THEN
    			call(data := stream.readable, done_ := done_);
    			IF call.DONE THEN
    				value := call.value;
    				ENDED := TRUE;
    			END_IF;
    		END_IF;
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Writes text to `push` through a stream of bytes it opens at the first
       request. */
    FUNCTION_BLOCK PUSH_WRITER
    VAR_INPUT
    	/* A rising edge writes `IN`; the first opens the stream and starts `push`
    	   with it. An edge while `BUSY` is ignored. */
    	REQ : BOOL;
    	/* The text to write. */
    	IN : STRING[4096];
    	/* A rising edge ends the stream, once no write is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    	/* TRUE once `push` has returned. */
    	ENDED : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	stream : BYTES_STREAM_WRITER;
    	call : Acme.Example.Pipe.PUSH;
    END_VAR
    	// The stream, made at the first request.
    	IF REQ AND NOT open THEN
    		stream();
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// The call, started with the stream's reading end at the first
    		// request and followed to its end, once.
    		IF NOT ENDED THEN
    			call(data := stream.readable);
    			IF call.DONE THEN
    				ENDED := TRUE;
    			END_IF;
    		END_IF;
    		stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    		BUSY := stream.BUSY;
    		DONE := stream.DONE;
    		ERROR := stream.ERROR;
    	END_IF;
    END_FUNCTION_BLOCK
    ");
}

/// Each value a writer keeps says what it holds in WIT's terms: a result's
/// flag and value, an option's flag and value, a tuple's items, and a
/// result with a value and an error, read out of words, as its parts.
#[rstest]
fn a_writer_s_outputs_say_what_they_hold_in_wit(mut with_db: RootDatabase) {
    let bindings = generated(
        r#"
        package acme:example@0.3.0;

        interface answers {
            enum error-code { full, gone }
            measure: func(data: stream<u8>) -> future<result<u32>>;
            maybe: func(data: stream<u8>) -> future<option<u32>>;
            pair: func(data: stream<u8>) -> future<tuple<u32, u8>>;
            sized: async func(data: stream<u8>) -> result<u64, error-code>;
        }

        world example {
            import answers;
        }
    "#,
    );
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    // The outputs of the writers of the functions.
    let outputs: String = bindings
        .files
        .iter()
        .flat_map(|file| file.text.split("\n\n"))
        .filter(|block| block.contains("_WRITER\n") && !block.contains("BYTES_STREAM_WRITER\n"))
        .filter_map(|block| {
            let name = block
                .lines()
                .find(|line| line.trim_start().starts_with("FUNCTION_BLOCK"))?;
            let outputs = block
                .split("VAR_OUTPUT\n")
                .nth(1)?
                .split("END_VAR")
                .next()?;
            Some(format!("{name}\n{outputs}\n"))
        })
        .collect();
    assert_snapshot!(outputs, @r"
    FUNCTION_BLOCK MEASURE_WRITER
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, a request comes after `CLOSE`,
    	   or the `result<u32>` from `measure` is an error. */
    	ERROR : BOOL;
    	/* TRUE once the `future<result<u32>>` from `measure` has its value. */
    	ENDED : BOOL;
    	/* The `u32` of the `result<u32>` from `measure`, when it is no error. */
    	value : UDINT;

    FUNCTION_BLOCK MAYBE_WRITER
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    	/* TRUE once the `future<option<u32>>` from `maybe` has its value. */
    	ENDED : BOOL;
    	/* TRUE when the `option<u32>` from `maybe` holds a value. */
    	is_some : BOOL;
    	/* The `u32` that the `option<u32>` from `maybe` holds. */
    	value : UDINT;

    FUNCTION_BLOCK PAIR_WRITER
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, or a request comes after
    	   `CLOSE`: the stream does not open again. */
    	ERROR : BOOL;
    	/* TRUE once the `future<tuple<u32, u8>>` from `pair` has its value. */
    	ENDED : BOOL;
    	/* Item 0 of the `tuple<u32, u8>` from `pair`, a `u32`. */
    	item0 : UDINT;
    	/* Item 1 of the `tuple<u32, u8>` from `pair`, a `u8`. */
    	item1 : USINT;

    FUNCTION_BLOCK SIZED_WRITER
    	/* TRUE while the reader has not taken all of `IN`. */
    	BUSY : BOOL;
    	/* TRUE in the call where the reader has taken all of `IN`, at once for an
    	   empty text. */
    	DONE : BOOL;
    	/* TRUE once a write finds the reader gone, a request comes after `CLOSE`,
    	   or the `result<u64, error-code>` from `sized` is an error, whose
    	   `error-code` is in `ERRORID`. */
    	ERROR : BOOL;
    	/* The `error-code` of the `result<u64, error-code>` from `sized`, when it
    	   is an error, which turns `ERROR` TRUE. */
    	ERRORID : ERROR_CODE;
    	/* TRUE once `sized` has returned. */
    	ENDED : BOOL;
    	/* The `u64` of the `result<u64, error-code>` from `sized`, when it is no
    	   error. */
    	value : ULINT;
    ");
}

/// A `func` that takes no stream or future and gives one stream of bytes,
/// alone or in a tuple beside one future, has a reader: `tail`, `fetch`,
/// whose tuple has its future first, a resource's `receive`, and stdin's
/// shape, `read-via-stream`, a result's error its `ERROR` and `ERRORID`;
/// the future of `receive` holds a string, which no read gives back, and
/// goes unread; `open`, named as the reader's own `open`, is called by its
/// namespace's path. Two streams, a future taken, a stream of numbers and
/// an `async func` have none.
#[rstest]
fn a_function_that_gives_a_stream_has_a_reader(mut with_db: RootDatabase) {
    let bindings = generated(
        r#"
        package acme:example@0.3.0;

        interface source {
            enum error-code { gone, full }
            resource socket {
                receive: func() -> tuple<stream<u8>, future<result<_, string>>>;
            }
            read-via-stream: func() -> tuple<stream<u8>, future<result<_, error-code>>>;
            tail: func() -> stream<u8>;
            open: func() -> stream<u8>;
            fetch: func(offset: u64) -> tuple<future<u32>, stream<u8>>;
            split: func() -> tuple<stream<u8>, stream<u8>>;
            relay: func(later: future<u32>) -> stream<u8>;
            numbers: func() -> stream<u32>;
            later: async func() -> stream<u8>;
        }

        world example {
            import source;
        }
    "#,
    );
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    let readers: String = bindings
        .files
        .iter()
        .flat_map(|file| file.text.split("\n\n"))
        .filter(|block| block.contains("_READER\n"))
        .filter_map(|block| block.split_inclusive("END_FUNCTION_BLOCK").next())
        .map(|block| format!("{}\n\n", block.trim_matches('\n')))
        .collect();
    assert_snapshot!(readers, @r"
    /* Reads what `socket.receive` gives through a stream of bytes, a request
       at a time. */
    FUNCTION_BLOCK SOCKET_RECEIVE_READER
    VAR_INPUT
    	/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    	   `socket.receive`. An edge while `BUSY` is ignored, and one after the
    	   stream ended is `DONE` at once, `OUT` empty. */
    	REQ : BOOL;
    	/* A rising edge lets go of the stream, once no read is under way. */
    	CLOSE : BOOL;
    	/* `self: borrow<socket>` of `socket.receive`, passed when the stream
    	   opens. */
    	self : SOCKET;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while a read waits for bytes. */
    	BUSY : BOOL;
    	/* TRUE in the call where a read has given what came, in `OUT`. */
    	DONE : BOOL;
    	/* What the last read gave: empty once the stream has ended, nothing more
    	   coming. */
    	OUT : STRING[4096];
    	/* TRUE once the stream has ended: the writer let go, or `CLOSE`. */
    	ENDED : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	requested : BOOL;
    	closing : BOOL;
    	shut : BOOL;
    	stream : UDINT;
    	future : UDINT;
    	read : BYTES_STREAM_READ;
    END_VAR
    	DONE := FALSE;
    	// The stream, given by the function at the first request.
    	IF REQ AND NOT open THEN
    		Acme.Example.Source.SOCKET_RECEIVE(self := self, item0 => stream, item1 => future);
    		// No read gives the future's value back: it goes unread.
    		RESULT_FUTURE_DROP_READABLE(future := future);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// An edge while BUSY is ignored.
    		IF REQ AND NOT requested AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BUSY := TRUE;
    			ELSE
    				// The stream has ended: nothing more comes.
    				OUT := '';
    				DONE := TRUE;
    			END_IF;
    		END_IF;
    		requested := REQ;
    		IF BUSY THEN
    			// Starts the read, or checks the one running.
    			read(stream := stream);
    			IF read.DONE THEN
    				OUT := read.data;
    				BUSY := FALSE;
    				DONE := TRUE;
    				IF read.closed THEN
    					// The writer left: the reading end goes too.
    					BYTES_STREAM_DROP_READABLE(stream := stream);
    					stream := 0;
    				END_IF;
    			END_IF;
    		END_IF;
    		// The reading end goes on CLOSE, once no read is under way.
    		IF CLOSE AND NOT closing THEN
    			shut := TRUE;
    		END_IF;
    		closing := CLOSE;
    		IF shut AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BYTES_STREAM_DROP_READABLE(stream := stream);
    				stream := 0;
    			END_IF;
    			shut := FALSE;
    		END_IF;
    		ENDED := stream = 0;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Reads what `read-via-stream` gives through a stream of bytes, a request
       at a time. */
    FUNCTION_BLOCK READ_VIA_STREAM_READER
    VAR_INPUT
    	/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    	   `read-via-stream`. An edge while `BUSY` is ignored, and one after the
    	   stream ended is `DONE` at once, `OUT` empty. */
    	REQ : BOOL;
    	/* A rising edge lets go of the stream, once no read is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while a read waits for bytes. */
    	BUSY : BOOL;
    	/* TRUE in the call where a read has given what came, in `OUT`. */
    	DONE : BOOL;
    	/* What the last read gave: empty once the stream has ended, nothing more
    	   coming. */
    	OUT : STRING[4096];
    	/* TRUE once the `result<_, error-code>` from `read-via-stream` is an
    	   error, whose `error-code` is in `ERRORID`. */
    	ERROR : BOOL;
    	/* The `error-code` of the `result<_, error-code>` from `read-via-stream`,
    	   when it is an error, which turns `ERROR` TRUE. */
    	ERRORID : ERROR_CODE;
    	/* TRUE once the stream has ended and the `future<result<_, error-code>>`
    	   from `read-via-stream` has its value. */
    	ENDED : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	requested : BOOL;
    	closing : BOOL;
    	shut : BOOL;
    	stream : UDINT;
    	future : UDINT;
    	read : BYTES_STREAM_READ;
    	answer : RESULT_FUTURE_GET;
    	came : BOOL;
    END_VAR
    	DONE := FALSE;
    	// The stream, given by the function at the first request.
    	IF REQ AND NOT open THEN
    		Acme.Example.Source.READ_VIA_STREAM(item0 => stream, item1 => future);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// An edge while BUSY is ignored.
    		IF REQ AND NOT requested AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BUSY := TRUE;
    			ELSE
    				// The stream has ended: nothing more comes.
    				OUT := '';
    				DONE := TRUE;
    			END_IF;
    		END_IF;
    		requested := REQ;
    		IF BUSY THEN
    			// Starts the read, or checks the one running.
    			read(stream := stream);
    			IF read.DONE THEN
    				OUT := read.data;
    				BUSY := FALSE;
    				DONE := TRUE;
    				IF read.closed THEN
    					// The writer left: the reading end goes too.
    					BYTES_STREAM_DROP_READABLE(stream := stream);
    					stream := 0;
    				END_IF;
    			END_IF;
    		END_IF;
    		// The reading end goes on CLOSE, once no read is under way.
    		IF CLOSE AND NOT closing THEN
    			shut := TRUE;
    		END_IF;
    		closing := CLOSE;
    		IF shut AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BYTES_STREAM_DROP_READABLE(stream := stream);
    				stream := 0;
    			END_IF;
    			shut := FALSE;
    		END_IF;
    		// The answer, read once it comes, and kept.
    		answer(future := future);
    		IF answer.DONE THEN
    			ERRORID := answer.error;
    			came := TRUE;
    		END_IF;
    		ENDED := stream = 0 AND came;
    		ERROR := answer.failed;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Reads what `tail` gives through a stream of bytes, a request at a time. */
    FUNCTION_BLOCK TAIL_READER
    VAR_INPUT
    	/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    	   `tail`. An edge while `BUSY` is ignored, and one after the stream ended
    	   is `DONE` at once, `OUT` empty. */
    	REQ : BOOL;
    	/* A rising edge lets go of the stream, once no read is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while a read waits for bytes. */
    	BUSY : BOOL;
    	/* TRUE in the call where a read has given what came, in `OUT`. */
    	DONE : BOOL;
    	/* What the last read gave: empty once the stream has ended, nothing more
    	   coming. */
    	OUT : STRING[4096];
    	/* TRUE once the stream has ended: the writer let go, or `CLOSE`. */
    	ENDED : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	requested : BOOL;
    	closing : BOOL;
    	shut : BOOL;
    	stream : UDINT;
    	read : BYTES_STREAM_READ;
    END_VAR
    	DONE := FALSE;
    	// The stream, given by the function at the first request.
    	IF REQ AND NOT open THEN
    		stream := Acme.Example.Source.TAIL();
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// An edge while BUSY is ignored.
    		IF REQ AND NOT requested AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BUSY := TRUE;
    			ELSE
    				// The stream has ended: nothing more comes.
    				OUT := '';
    				DONE := TRUE;
    			END_IF;
    		END_IF;
    		requested := REQ;
    		IF BUSY THEN
    			// Starts the read, or checks the one running.
    			read(stream := stream);
    			IF read.DONE THEN
    				OUT := read.data;
    				BUSY := FALSE;
    				DONE := TRUE;
    				IF read.closed THEN
    					// The writer left: the reading end goes too.
    					BYTES_STREAM_DROP_READABLE(stream := stream);
    					stream := 0;
    				END_IF;
    			END_IF;
    		END_IF;
    		// The reading end goes on CLOSE, once no read is under way.
    		IF CLOSE AND NOT closing THEN
    			shut := TRUE;
    		END_IF;
    		closing := CLOSE;
    		IF shut AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BYTES_STREAM_DROP_READABLE(stream := stream);
    				stream := 0;
    			END_IF;
    			shut := FALSE;
    		END_IF;
    		ENDED := stream = 0;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Reads what `open` gives through a stream of bytes, a request at a time. */
    FUNCTION_BLOCK OPEN_READER
    VAR_INPUT
    	/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    	   `open`. An edge while `BUSY` is ignored, and one after the stream ended
    	   is `DONE` at once, `OUT` empty. */
    	REQ : BOOL;
    	/* A rising edge lets go of the stream, once no read is under way. */
    	CLOSE : BOOL;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while a read waits for bytes. */
    	BUSY : BOOL;
    	/* TRUE in the call where a read has given what came, in `OUT`. */
    	DONE : BOOL;
    	/* What the last read gave: empty once the stream has ended, nothing more
    	   coming. */
    	OUT : STRING[4096];
    	/* TRUE once the stream has ended: the writer let go, or `CLOSE`. */
    	ENDED : BOOL;
    END_VAR
    VAR
    	open : BOOL;
    	requested : BOOL;
    	closing : BOOL;
    	shut : BOOL;
    	stream : UDINT;
    	read : BYTES_STREAM_READ;
    END_VAR
    	DONE := FALSE;
    	// The stream, given by the function at the first request.
    	IF REQ AND NOT open THEN
    		stream := Acme.Example.Source.OPEN();
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// An edge while BUSY is ignored.
    		IF REQ AND NOT requested AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BUSY := TRUE;
    			ELSE
    				// The stream has ended: nothing more comes.
    				OUT := '';
    				DONE := TRUE;
    			END_IF;
    		END_IF;
    		requested := REQ;
    		IF BUSY THEN
    			// Starts the read, or checks the one running.
    			read(stream := stream);
    			IF read.DONE THEN
    				OUT := read.data;
    				BUSY := FALSE;
    				DONE := TRUE;
    				IF read.closed THEN
    					// The writer left: the reading end goes too.
    					BYTES_STREAM_DROP_READABLE(stream := stream);
    					stream := 0;
    				END_IF;
    			END_IF;
    		END_IF;
    		// The reading end goes on CLOSE, once no read is under way.
    		IF CLOSE AND NOT closing THEN
    			shut := TRUE;
    		END_IF;
    		closing := CLOSE;
    		IF shut AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BYTES_STREAM_DROP_READABLE(stream := stream);
    				stream := 0;
    			END_IF;
    			shut := FALSE;
    		END_IF;
    		ENDED := stream = 0;
    	END_IF;
    END_FUNCTION_BLOCK

    /* Reads what `fetch` gives through a stream of bytes, a request at a time. */
    FUNCTION_BLOCK FETCH_READER
    VAR_INPUT
    	/* A rising edge reads what comes next, 4096 bytes at most; the first calls
    	   `fetch`. An edge while `BUSY` is ignored, and one after the stream ended
    	   is `DONE` at once, `OUT` empty. */
    	REQ : BOOL;
    	/* A rising edge lets go of the stream, once no read is under way. */
    	CLOSE : BOOL;
    	/* `offset: u64` of `fetch`, passed when the stream opens. */
    	offset : ULINT;
    END_VAR
    VAR_OUTPUT
    	/* TRUE while a read waits for bytes. */
    	BUSY : BOOL;
    	/* TRUE in the call where a read has given what came, in `OUT`. */
    	DONE : BOOL;
    	/* What the last read gave: empty once the stream has ended, nothing more
    	   coming. */
    	OUT : STRING[4096];
    	/* TRUE once the stream has ended and the `future<u32>` from `fetch` has
    	   its value. */
    	ENDED : BOOL;
    	/* The `u32` from `fetch`. */
    	value : UDINT;
    END_VAR
    VAR
    	open : BOOL;
    	requested : BOOL;
    	closing : BOOL;
    	shut : BOOL;
    	stream : UDINT;
    	future : UDINT;
    	read : BYTES_STREAM_READ;
    	answer : U32_FUTURE_GET;
    	came : BOOL;
    END_VAR
    	DONE := FALSE;
    	// The stream, given by the function at the first request.
    	IF REQ AND NOT open THEN
    		Acme.Example.Source.FETCH(offset := offset, item1 => stream, item0 => future);
    		open := TRUE;
    	END_IF;
    	IF open THEN
    		// An edge while BUSY is ignored.
    		IF REQ AND NOT requested AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BUSY := TRUE;
    			ELSE
    				// The stream has ended: nothing more comes.
    				OUT := '';
    				DONE := TRUE;
    			END_IF;
    		END_IF;
    		requested := REQ;
    		IF BUSY THEN
    			// Starts the read, or checks the one running.
    			read(stream := stream);
    			IF read.DONE THEN
    				OUT := read.data;
    				BUSY := FALSE;
    				DONE := TRUE;
    				IF read.closed THEN
    					// The writer left: the reading end goes too.
    					BYTES_STREAM_DROP_READABLE(stream := stream);
    					stream := 0;
    				END_IF;
    			END_IF;
    		END_IF;
    		// The reading end goes on CLOSE, once no read is under way.
    		IF CLOSE AND NOT closing THEN
    			shut := TRUE;
    		END_IF;
    		closing := CLOSE;
    		IF shut AND NOT BUSY THEN
    			IF stream <> 0 THEN
    				BYTES_STREAM_DROP_READABLE(stream := stream);
    				stream := 0;
    			END_IF;
    			shut := FALSE;
    		END_IF;
    		// The answer, read once it comes, and kept.
    		answer(future := future);
    		IF answer.DONE THEN
    			value := answer.value;
    			came := TRUE;
    		END_IF;
    		ENDED := stream = 0 AND came;
    	END_IF;
    END_FUNCTION_BLOCK
    ");
}

/// A future's value comes back as a function's would: nothing, a scalar,
/// a flag and a side of a result, a flag and an option's value, a record
/// in its STRUCT, whose fields the import gives as its outputs; a result
/// with both sides in words, which its read is read out of. A string is
/// not read yet.
#[rstest]
fn a_future_s_value_comes_back_as_outputs(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

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
    // acme-example-promises.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Promises
    			USING Std.Convert;

    			TYPE ERROR_CODE : USINT (
    				LOST := 0
    			); END_TYPE

    			TYPE POINT : STRUCT
    				x : LREAL;
    				y : LREAL;
    			END_STRUCT END_TYPE

    			{extern 'acme:example/promises@0.3.0' 'ready'}
    			FUNCTION READY : UDINT
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]ready'}
    			FUNCTION_BLOCK UNIT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]ready'}
    			FUNCTION UNIT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK UNIT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				call : UNIT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						UNIT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/promises@0.3.0' 'count'}
    			FUNCTION COUNT : UDINT
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]count'}
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
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]count'}
    			FUNCTION U32_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK U32_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : UDINT;
    			END_VAR
    			VAR
    				call : U32_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						value := call.value;
    						U32_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/promises@0.3.0' 'outcome'}
    			FUNCTION OUTCOME : UDINT
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]outcome'}
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
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]outcome'}
    			FUNCTION RESULT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK RESULT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						failed := call.failed;
    						error := call.error;
    						RESULT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/promises@0.3.0' 'maybe'}
    			FUNCTION MAYBE : UDINT
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]maybe'}
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
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]maybe'}
    			FUNCTION OPTION_S64_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK OPTION_S64_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				is_some : BOOL;
    				value : LINT;
    			END_VAR
    			VAR
    				call : OPTION_S64_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						is_some := call.is_some;
    						value := call.value;
    						OPTION_S64_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/promises@0.3.0' 'where-is'}
    			FUNCTION WHERE_IS : UDINT
    			END_FUNCTION

    			/* The future's read as the host gives it: the values the future holds,
    			   which `POINT_FUTURE_READ` puts in the members of `value`. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]where-is'}
    			FUNCTION_BLOCK POINT_FUTURE_READ_RAW
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				x : LREAL;
    				y : LREAL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			FUNCTION_BLOCK POINT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : POINT;
    			END_VAR
    			VAR
    				call : POINT_FUTURE_READ_RAW;
    			END_VAR
    				call(future := future);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.x := call.x;
    					value.y := call.y;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]where-is'}
    			FUNCTION POINT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK POINT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : POINT;
    			END_VAR
    			VAR
    				call : POINT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						value := call.value;
    						POINT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/promises@0.3.0' 'name'}
    			FUNCTION NAME : UDINT
    			END_FUNCTION

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]name'}
    			FUNCTION STRING_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'acme:example/promises@0.3.0' 'both'}
    			FUNCTION BOTH : UDINT
    			END_FUNCTION

    			/* The future's read as the host gives it: its value in words of 8 bytes,
    			   as the canonical ABI lays it out, which `RESULT_U64_FUTURE_READ` reads. */
    			{extern async 'acme:example/promises@0.3.0' '[future-read-0]both'}
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
    			{extern 'acme:example/promises@0.3.0' '[future-drop-readable-0]both'}
    			FUNCTION RESULT_U64_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK RESULT_U64_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				value : ULINT;
    				error : ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_U64_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						failed := call.failed;
    						value := call.value;
    						error := call.error;
    						RESULT_U64_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `UNIT_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `U32_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `OPTION_S64_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `POINT_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `STRING_FUTURE_READ`: its value holds a string, which a read cannot
         give back yet.
       - `STRING_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `RESULT_U64_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
    ");
}

/// A built-in whose name a function of the interface takes gives way to
/// it, with a `_` after its own: the generator would otherwise write two
/// POUs of one name.
#[rstest]
fn a_built_in_named_like_a_function_gives_way(mut with_db: RootDatabase) {
    let bindings = generated(
        r#"
        package acme:example@0.3.0;

        interface feeds {
            record event { x: u32 }
            event-stream-drop-readable: func() -> u32;
            feed: func() -> stream<event>;
        }

        world example {
            import feeds;
        }
    "#,
    );
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    assert_snapshot!(summary(&bindings.files), @r"
    acme-example-feeds.st: EVENT, EVENT_STREAM_DROP_READABLE, FEED, EVENT_STREAM_DROP_READABLE_
        left out `EVENT_STREAM_READ`: a stream's read moves bytes or handles only
        left out `EVENT_STREAM_CANCEL_READ`: a cancel blocks the scan
    ");
}

/// A future's value whose fields are named as the names its block has:
/// `done` beside the block's `DONE`, and `call` beside the block that
/// holds the import's. Each is a member of the value under its own name.
/// Where the import gives the fields as its outputs, `done` takes a `_`
/// there, beside the import's own `DONE`, and `call` keeps its name.
#[rstest]
fn a_future_s_value_keeps_away_from_its_block_s_names(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

        interface progress {
            record state { done: bool, call: u32 }
            variant phase { idle, moved(u32) }
            record report { done: bool, call: u32, phase: phase }
            watch: func() -> future<state>;
            follow: func() -> future<report>;
        }

        world example {
            import progress;
        }
    "#), @r"
    // acme-example-progress.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Progress
    			USING Std.Convert;

    			TYPE STATE : STRUCT
    				done : BOOL;
    				call : UDINT;
    			END_STRUCT END_TYPE

    			/* The cases of `phase`. */
    			TYPE PHASE_KIND : USINT (
    				IDLE := 0,
    				MOVED := 1
    			); END_TYPE

    			TYPE PHASE : STRUCT
    				kind : PHASE_KIND;
    				moved : UDINT;
    			END_STRUCT END_TYPE

    			TYPE REPORT : STRUCT
    				done : BOOL;
    				call : UDINT;
    				phase : PHASE;
    			END_STRUCT END_TYPE

    			{extern 'acme:example/progress@0.3.0' 'watch'}
    			FUNCTION WATCH : UDINT
    			END_FUNCTION

    			/* The future's read as the host gives it: the values the future holds,
    			   which `STATE_FUTURE_READ` puts in the members of `value`. */
    			{extern async 'acme:example/progress@0.3.0' '[future-read-0]watch'}
    			FUNCTION_BLOCK STATE_FUTURE_READ_RAW
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				done_ : BOOL;
    				call : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* Reads the value the future holds, which comes back after `DONE`. */
    			FUNCTION_BLOCK STATE_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : STATE;
    			END_VAR
    			VAR
    				call : STATE_FUTURE_READ_RAW;
    			END_VAR
    				call(future := future);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.done := call.done_;
    					value.call := call.call;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/progress@0.3.0' '[future-drop-readable-0]watch'}
    			FUNCTION STATE_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK STATE_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : STATE;
    			END_VAR
    			VAR
    				call : STATE_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						value := call.value;
    						STATE_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern 'acme:example/progress@0.3.0' 'follow'}
    			FUNCTION FOLLOW : UDINT
    			END_FUNCTION

    			/* The future's read as the host gives it: its value in words of 8 bytes,
    			   as the canonical ABI lays it out, which `REPORT_FUTURE_READ` reads. */
    			{extern async 'acme:example/progress@0.3.0' '[future-read-0]follow'}
    			FUNCTION_BLOCK REPORT_FUTURE_READ_RAW
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
    			FUNCTION_BLOCK REPORT_FUTURE_READ
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : REPORT;
    			END_VAR
    			VAR
    				call : REPORT_FUTURE_READ_RAW;
    			END_VAR
    				call(future := future);
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.phase.moved := 0;
    					value.done := call.w0.%X0;
    					value.call := DWORD_TO_UDINT(call.w0.%D1);
    					value.phase.kind := BYTE_TO_USINT(call.w1.%B0);
    					CASE value.phase.kind OF
    						PHASE_KIND#MOVED:
    							value.phase.moved := DWORD_TO_UDINT(call.w1.%D1);
    					END_CASE;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Lets go of the future's reading end, `future`. */
    			{extern 'acme:example/progress@0.3.0' '[future-drop-readable-0]follow'}
    			FUNCTION REPORT_FUTURE_DROP_READABLE
    			VAR_INPUT
    				future : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK REPORT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : REPORT;
    			END_VAR
    			VAR
    				call : REPORT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						value := call.value;
    						REPORT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `STATE_FUTURE_CANCEL_READ`: a cancel blocks the scan.
       - `REPORT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
    ");
}

/// The built-ins of Std's standard output: a stream of bytes and the
/// future it answers with, named after its `write-via-stream`, as the
/// standard error's are after its own.
#[test]
fn std_s_built_ins() {
    let bindings = generated_std();
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

    			/* Writes the `count` bytes from the one `data` points at, as many as the
    			   reader takes: `written` of them, after `DONE`. The host reads them where
    			   they are while the write runs, so they stay there, unchanged, until
    			   `DONE`. `closed` is TRUE once the reader has let go. */
    			{extern async 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
    			FUNCTION_BLOCK BYTES_STREAM_WRITE_ARRAY
    			VAR_INPUT
    				stream : UDINT;
    				data : REF_TO BYTE;
    				count : UDINT;
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

    			/* The bytes `IN` holds, which `BYTES_STREAM_WRITER` counts. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_LEN : UDINT
    			VAR_INPUT
    				IN : STRING[4096];
    			END_VAR
    				{wasm 'str.byte_len' (params IN) (result BYTES_STREAM_TEXT_LEN)}
    			END_FUNCTION

    			/* `IN` without its first `L` bytes: what `BYTES_STREAM_WRITER` has left to
    			   write. */
    			FUNCTION PRIVATE BYTES_STREAM_TEXT_AFTER : STRING[4096]
    			VAR_INPUT
    				IN : STRING[4096];
    				L : UDINT;
    			END_VAR
    			VAR
    				P : UDINT := 1;
    			END_VAR
    				{wasm 'str.byte_delete' (params IN L P) (result BYTES_STREAM_TEXT_AFTER)}
    			END_FUNCTION

    			/* Writes text to a stream of bytes it makes at its first call, for the
    			   function `readable` is handed to. */
    			FUNCTION_BLOCK BYTES_STREAM_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* The stream's reading end, to hand to the function that reads it. */
    				readable : UDINT;
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, or a request comes after
    				   `CLOSE`: the stream does not open again. */
    				ERROR : BOOL;
    			END_VAR
    			VAR
    				writable : UDINT;
    				requested : BOOL;
    				closing : BOOL;
    				shut : BOOL;
    				closed : BOOL;
    				rest : STRING[4096];
    				write : BYTES_STREAM_WRITE;
    			END_VAR
    			VAR_TEMP
    				again : BOOL;
    			END_VAR
    				IF writable = 0 AND NOT closed THEN
    					BYTES_STREAM_NEW(readable => readable, writable => writable);
    				END_IF;
    				DONE := FALSE;
    				// An edge while BUSY is ignored.
    				IF REQ AND NOT requested AND NOT BUSY THEN
    					ERROR := closed;
    					IF NOT closed THEN
    						// A write of no bytes asks the reader whether it is ready,
    						// which it may answer late: an empty text is done here.
    						IF BYTES_STREAM_TEXT_LEN(IN) = 0 THEN
    							DONE := TRUE;
    						ELSE
    							rest := IN;
    							BUSY := TRUE;
    						END_IF;
    					END_IF;
    				END_IF;
    				requested := REQ;
    				IF BUSY THEN
    					// Each pass after the first hands over a byte at least, so the
    					// loop ends.
    					REPEAT
    						again := FALSE;
    						// Starts the write of the rest, or checks the one running.
    						write(stream := writable, data := rest);
    						IF write.DONE THEN
    							IF write.closed THEN
    								// The reader left: the writing end goes too.
    								BYTES_STREAM_DROP_WRITABLE(stream := writable);
    								writable := 0;
    								closed := TRUE;
    								BUSY := FALSE;
    								ERROR := TRUE;
    							ELSIF write.written < BYTES_STREAM_TEXT_LEN(rest) THEN
    								// The reader took a part: the rest goes next, at once while
    								// it takes some.
    								rest := BYTES_STREAM_TEXT_AFTER(rest, write.written);
    								again := write.written > 0;
    							ELSE
    								BUSY := FALSE;
    								DONE := TRUE;
    							END_IF;
    						END_IF;
    					UNTIL NOT again
    					END_REPEAT;
    				END_IF;
    				// The writing end goes once no write is under way.
    				IF CLOSE AND NOT closing THEN
    					shut := TRUE;
    				END_IF;
    				closing := CLOSE;
    				IF shut AND NOT BUSY THEN
    					IF writable <> 0 THEN
    						BYTES_STREAM_DROP_WRITABLE(stream := writable);
    						writable := 0;
    					END_IF;
    					closed := TRUE;
    					shut := FALSE;
    				END_IF;
    			END_FUNCTION_BLOCK

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

    			/* Reads the value the future holds, which comes back after `DONE`, then
    			   lets go of the future's reading end: `future` is 0 from then on, and a
    			   call with `future` 0 does nothing. */
    			FUNCTION_BLOCK RESULT_FUTURE_GET
    			VAR_IN_OUT
    				future : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : Wasi.Cli.Types.ERROR_CODE;
    			END_VAR
    			VAR
    				call : RESULT_FUTURE_READ;
    			END_VAR
    				DONE := FALSE;
    				IF future <> 0 THEN
    					call(future := future);
    					IF call.DONE THEN
    						failed := call.failed;
    						error := call.error;
    						RESULT_FUTURE_DROP_READABLE(future := future);
    						future := 0;
    						DONE := TRUE;
    					END_IF;
    				END_IF;
    			END_FUNCTION_BLOCK

    			/* Writes text to `write-via-stream` through a stream of bytes it opens at
    			   the first request. */
    			FUNCTION_BLOCK WRITE_VIA_STREAM_WRITER
    			VAR_INPUT
    				/* A rising edge writes `IN`; the first opens the stream and calls
    				   `write-via-stream` with it. An edge while `BUSY` is ignored. */
    				REQ : BOOL;
    				/* The text to write. */
    				IN : STRING[4096];
    				/* A rising edge ends the stream, once no write is under way. */
    				CLOSE : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				/* TRUE while the reader has not taken all of `IN`. */
    				BUSY : BOOL;
    				/* TRUE in the call where the reader has taken all of `IN`, at once for an
    				   empty text. */
    				DONE : BOOL;
    				/* TRUE once a write finds the reader gone, a request comes after `CLOSE`,
    				   or the `result<_, error-code>` from `write-via-stream` is an error,
    				   whose `error-code` is in `ERRORID`. */
    				ERROR : BOOL;
    				/* The `error-code` of the `result<_, error-code>` from `write-via-stream`,
    				   when it is an error, which turns `ERROR` TRUE. */
    				ERRORID : Wasi.Cli.Types.ERROR_CODE;
    				/* TRUE once the `future<result<_, error-code>>` from `write-via-stream`
    				   has its value. */
    				ENDED : BOOL;
    			END_VAR
    			VAR
    				open : BOOL;
    				stream : BYTES_STREAM_WRITER;
    				future : UDINT;
    				answer : RESULT_FUTURE_GET;
    			END_VAR
    				// The stream, made at the first request and handed to the function.
    				IF REQ AND NOT open THEN
    					stream();
    					future := Wasi.Cli.Stdout.WRITE_VIA_STREAM(data := stream.readable);
    					open := TRUE;
    				END_IF;
    				IF open THEN
    					stream(REQ := REQ, IN := IN, CLOSE := CLOSE);
    					BUSY := stream.BUSY;
    					DONE := stream.DONE;
    					// The answer, read once it comes, and kept.
    					answer(future := future);
    					IF answer.DONE THEN
    						ERRORID := answer.error;
    						ENDED := TRUE;
    					END_IF;
    					ERROR := stream.ERROR OR answer.failed;
    				END_IF;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan.
       - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
    ");
}
