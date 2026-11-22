/* Generated from the WIT of wasi:cli@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Cli
		NAMESPACE Stdin
			/* Return a stream for reading from stdin.

			   This function returns a stream which provides data read from stdin,
			   and a future to signal read results.

			   If the stream's readable end is dropped the future will resolve to success.

			   If the stream's writable end is dropped the future will either resolve to
			   success if stdin was closed by the writer or to an error-code if reading
			   failed for some other reason.

			   Multiple streams may be active at the same time. The behavior of concurrent
			   reads is implementation-specific. */
			{extern 'wasi:cli/stdin@0.3.0' 'read-via-stream'}
			FUNCTION READ_VIA_STREAM
			VAR_OUTPUT
				item0 : UDINT;
				item1 : UDINT;
			END_VAR
			END_FUNCTION

			/* Reads the bytes that come next into `data`, as many as it holds at most:
			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
			   go. */
			{extern async 'wasi:cli/stdin@0.3.0' '[stream-read-0]read-via-stream'}
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
			{extern async 'wasi:cli/stdin@0.3.0' '[stream-read-0]read-via-stream'}
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

			/* Lets go of the stream's reading end, `stream`. */
			{extern 'wasi:cli/stdin@0.3.0' '[stream-drop-readable-0]read-via-stream'}
			FUNCTION BYTES_STREAM_DROP_READABLE
			VAR_INPUT
				stream : UDINT;
			END_VAR
			END_FUNCTION

			/* Reads the value the future holds, which comes back after `DONE`. */
			{extern async 'wasi:cli/stdin@0.3.0' '[future-read-1]read-via-stream'}
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
			{extern 'wasi:cli/stdin@0.3.0' '[future-drop-readable-1]read-via-stream'}
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
				ERRORID : Wasi.Cli.Types.ERROR_CODE;
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
					Wasi.Cli.Stdin.READ_VIA_STREAM(item0 => stream, item1 => future);
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

			/* Reads what `read-via-stream` gives through a stream of bytes into an
			   array, a request at a time. */
			FUNCTION_BLOCK READ_VIA_STREAM_ARRAY_READER
			VAR_INPUT
				/* A rising edge reads what comes next, 8192 bytes at most; the first calls
				   `read-via-stream`. An edge while `BUSY` is ignored, and one after the
				   stream ended is `DONE` at once, `COUNT` 0. */
				REQ : BOOL;
				/* A rising edge lets go of the stream, once no read is under way. */
				CLOSE : BOOL;
			END_VAR
			VAR_OUTPUT
				/* TRUE while a read waits for bytes. */
				BUSY : BOOL;
				/* TRUE in the call where a read has given what came, in `OUT`. */
				DONE : BOOL;
				/* The bytes the last read gave, its first `COUNT`. */
				OUT : ARRAY[0..8191] OF BYTE;
				/* How many bytes the last read gave: 0 once the stream has ended, nothing
				   more coming. */
				COUNT : UDINT;
				/* TRUE once the `result<_, error-code>` from `read-via-stream` is an
				   error, whose `error-code` is in `ERRORID`. */
				ERROR : BOOL;
				/* The `error-code` of the `result<_, error-code>` from `read-via-stream`,
				   when it is an error, which turns `ERROR` TRUE. */
				ERRORID : Wasi.Cli.Types.ERROR_CODE;
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
				read : BYTES_STREAM_READ_ARRAY;
				answer : RESULT_FUTURE_GET;
				came : BOOL;
			END_VAR
				DONE := FALSE;
				// The stream, given by the function at the first request.
				IF REQ AND NOT open THEN
					Wasi.Cli.Stdin.READ_VIA_STREAM(item0 => stream, item1 => future);
					open := TRUE;
				END_IF;
				IF open THEN
					// An edge while BUSY is ignored.
					IF REQ AND NOT requested AND NOT BUSY THEN
						IF stream <> 0 THEN
							BUSY := TRUE;
						ELSE
							// The stream has ended: nothing more comes.
							COUNT := 0;
							DONE := TRUE;
						END_IF;
					END_IF;
					requested := REQ;
					IF BUSY THEN
						// Starts the read, or checks the one running.
						read(stream := stream);
						IF read.DONE THEN
							OUT := read.data;
							COUNT := read.read;
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
   - `BYTES_STREAM_CANCEL_READ`: a cancel blocks the scan.
   - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
