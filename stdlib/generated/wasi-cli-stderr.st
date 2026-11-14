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

			/* Reads the bytes that come next into `data`, as many as it holds at most:
			   `read` of them, after `DONE`. `closed` is TRUE once the writer has let
			   go. */
			{extern async 'wasi:cli/stderr@0.3.0' '[stream-read-0]write-via-stream'}
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
			{extern async 'wasi:cli/stderr@0.3.0' '[stream-read-0]write-via-stream'}
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

			/* Makes a future of `result<_, error-code>`, and gives its reading end,
			   then its writing end. */
			{extern 'wasi:cli/stderr@0.3.0' '[future-new-1]write-via-stream'}
			FUNCTION RESULT_FUTURE_NEW
			VAR_OUTPUT
				readable : UDINT;
				writable : UDINT;
			END_VAR
			END_FUNCTION

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

			/* Lets go of the future's writing end, `future`. */
			{extern 'wasi:cli/stderr@0.3.0' '[future-drop-writable-1]write-via-stream'}
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
