/* Generated from the WIT of wasi:cli@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Cli
		NAMESPACE Stderr
			/* Write the given stream to stderr.

			   If the stream's writable end is dropped this function will either return
			   success once the entire contents of the stream have been written or an
			   error-code representing a failure.

			   Otherwise if there is an error the readable end of the stream will be
			   dropped and this function will return an error-code. */
			{extern 'wasi:cli/stderr@0.3.0' 'write-via-stream'}
			FUNCTION WRITE_VIA_STREAM : UDINT
			VAR_INPUT
				data : UDINT;
			END_VAR
			END_FUNCTION

			/* Makes a stream of `u8`, and gives its reading end, then its writing end. */
			{extern 'wasi:cli/stderr@0.3.0' '[stream-new-0]write-via-stream'}
			FUNCTION BYTES_STREAM_NEW
			VAR_OUTPUT
				readable : UDINT;
				writable : UDINT;
			END_VAR
			END_FUNCTION

			/* Writes the bytes of `data`, as many as the reader takes: `written` of
			   them, after `DONE`. `closed` is TRUE once the reader has let go. */
			{extern async 'wasi:cli/stderr@0.3.0' '[stream-write-0]write-via-stream'}
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
			{extern async 'wasi:cli/stderr@0.3.0' '[stream-write-0]write-via-stream'}
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
			{extern 'wasi:cli/stderr@0.3.0' '[stream-drop-readable-0]write-via-stream'}
			FUNCTION BYTES_STREAM_DROP_READABLE
			VAR_INPUT
				stream : UDINT;
			END_VAR
			END_FUNCTION

			/* Lets go of the stream's writing end, `stream`. */
			{extern 'wasi:cli/stderr@0.3.0' '[stream-drop-writable-0]write-via-stream'}
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
			{extern async 'wasi:cli/stderr@0.3.0' '[future-read-1]write-via-stream'}
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
			{extern 'wasi:cli/stderr@0.3.0' '[future-drop-readable-1]write-via-stream'}
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
					future := Wasi.Cli.Stderr.WRITE_VIA_STREAM(data := stream.readable);
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
