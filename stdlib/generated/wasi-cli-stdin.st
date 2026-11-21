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
		END_NAMESPACE
	END_NAMESPACE
END_NAMESPACE
/* Not generated:
   - `BYTES_STREAM_CANCEL_READ`: a cancel blocks the scan.
   - `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan. */
