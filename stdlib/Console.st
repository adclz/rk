/*
The host's standard output and standard error.

`STDOUT` and `STDERR` write a `STRING` as it holds it, with no line break
added: end the text with `$N` for one. A rising edge of `REQ` writes `IN`,
once. `BUSY` is TRUE while the host takes it, and `DONE` in the call where
it has all of it, or at once for an empty text. A rising edge while `BUSY`
is ignored, as a block with `REQ` and `BUSY` ignores one: check `BUSY`
before asking again. `ERROR` is TRUE when the host has let go of its end:
the output stays closed, and each later request is an error at once. Why
the host let go is not kept.

Each block writes through a stream of bytes of its own, which it makes at
its first request and keeps, as WASI's
[standard output and error](https://github.com/WebAssembly/WASI/blob/main/proposals/cli/README.md)
take one. Two blocks hand the host two streams: the text of one comes out in
its order, but a text the host takes in parts can have the other block's
text land between them. `IN` holds 4096 bytes, and a longer `STRING` is cut
there, as any input is.

A scan never waits for the host. What it does not take within the call it
takes at later calls, while `BUSY` stays TRUE.
*/
NAMESPACE Std.Console

	/* The blocks reach WASI through what rk-bindgen writes in `generated/`
	   from WASI's WIT. Each output's `WRITE_VIA_STREAM`, in `Wasi.Cli.Stdout`
	   and in `Wasi.Cli.Stderr`, hands the host the reading end of a stream of
	   bytes, which the interface's `BYTES_STREAM_NEW` makes, and answers with
	   a future of how the writing went, which its
	   `RESULT_FUTURE_DROP_READABLE` gives up unread: its error code is not
	   kept, and a write only sees that the host let go. Both streams are
	   written with the standard output's `BYTES_STREAM_WRITE`: a write is the
	   component model's, and takes any stream of bytes. */

	/* What `STDOUT` and `STDERR` write with, which they alone reach. */
	NAMESPACE INTERNAL Streams

		/* The bytes `IN` holds. */
		FUNCTION TEXT_LEN: UDINT
			VAR_INPUT
				IN: STRING[4096];
			END_VAR
			{wasm 'str.byte_len' (params IN) (result TEXT_LEN)}
		END_FUNCTION

		/* `IN` without its first `L` bytes. */
		FUNCTION TEXT_AFTER: STRING[4096]
			VAR_INPUT
				IN: STRING[4096];
				L: UDINT;
			END_VAR
			VAR
				P: UDINT := 1;
			END_VAR
			{wasm 'str.byte_delete' (params IN L P) (result TEXT_AFTER)}
		END_FUNCTION

		/* Writes `IN` to the stream of bytes whose writing end is `stream` on
		   a rising edge of `REQ`, handing the host what it has not taken
		   until it has all of it: the work of `STDOUT` and `STDERR`, which
		   make the stream. */
		FUNCTION_BLOCK WRITER
			VAR_INPUT
				REQ: BOOL;
				IN: STRING[4096];
				stream: UDINT;
			END_VAR
			VAR_OUTPUT
				BUSY: BOOL;
				DONE: BOOL;
				ERROR: BOOL;
			END_VAR
			VAR
				requested: BOOL;
				closed: BOOL;
				rest: STRING[4096];
				write: Wasi.Cli.Stdout.BYTES_STREAM_WRITE;
			END_VAR
			VAR_TEMP
				again: BOOL;
			END_VAR

			DONE := FALSE;
			// An edge while BUSY is ignored.
			IF REQ AND NOT requested AND NOT BUSY THEN
				ERROR := closed;
				IF NOT closed THEN
					// A copy of no bytes asks the host whether it is ready,
					// which it may answer late: an empty text is done here.
					IF TEXT_LEN(IN) = 0 THEN
						DONE := TRUE;
					ELSE
						rest := IN;
						BUSY := TRUE;
					END_IF;
				END_IF;
			END_IF;
			requested := REQ;

			IF BUSY THEN
				// Each pass after the first hands over at least one byte, so
				// the loop ends.
				REPEAT
					again := FALSE;
					// Starts the copy of the rest, or checks the one running.
					write(stream := stream, data := rest);
					IF write.DONE THEN
						IF write.closed THEN
							closed := TRUE;
							BUSY := FALSE;
							ERROR := TRUE;
						ELSIF write.written < TEXT_LEN(rest) THEN
							// The host took a part: the rest goes next, at
							// once while it takes some.
							rest := TEXT_AFTER(rest, write.written);
							again := write.written > 0;
						ELSE
							BUSY := FALSE;
							DONE := TRUE;
						END_IF;
					END_IF;
				UNTIL NOT again
				END_REPEAT;
			END_IF;
		END_FUNCTION_BLOCK
	END_NAMESPACE

	/* Writes `IN` to the host's standard output on a rising edge of `REQ`.

	   # Examples

	   ```iecst
	   VAR
	       log: STDOUT;
	   END_VAR

	   log(REQ := pump_on, IN := 'pump started$N');
	   ```
	*/
	FUNCTION_BLOCK STDOUT
		VAR_INPUT
			REQ: BOOL;
			IN: STRING[4096];
		END_VAR
		VAR_OUTPUT
			BUSY: BOOL;
			DONE: BOOL;
			ERROR: BOOL;
		END_VAR
		VAR
			open: BOOL;
			writing: UDINT;
			output: Streams.WRITER;
		END_VAR
		VAR_TEMP
			reading: UDINT;
		END_VAR

		// The stream, made at the first request.
		IF REQ AND NOT open THEN
			Wasi.Cli.Stdout.BYTES_STREAM_NEW(readable => reading, writable => writing);
			Wasi.Cli.Stdout.RESULT_FUTURE_DROP_READABLE(
				future := Wasi.Cli.Stdout.WRITE_VIA_STREAM(data := reading));
			open := TRUE;
		END_IF;
		output(REQ := REQ, IN := IN, stream := writing);
		BUSY := output.BUSY;
		DONE := output.DONE;
		ERROR := output.ERROR;
	END_FUNCTION_BLOCK

	/* Writes `IN` to the host's standard error on a rising edge of `REQ`.

	   # Examples

	   ```iecst
	   VAR
	       alarm: STDERR;
	   END_VAR

	   alarm(REQ := pressure > 3.0, IN := 'pressure out of range$N');
	   ```
	*/
	FUNCTION_BLOCK STDERR
		VAR_INPUT
			REQ: BOOL;
			IN: STRING[4096];
		END_VAR
		VAR_OUTPUT
			BUSY: BOOL;
			DONE: BOOL;
			ERROR: BOOL;
		END_VAR
		VAR
			open: BOOL;
			writing: UDINT;
			output: Streams.WRITER;
		END_VAR
		VAR_TEMP
			reading: UDINT;
		END_VAR

		// The stream, made at the first request.
		IF REQ AND NOT open THEN
			Wasi.Cli.Stderr.BYTES_STREAM_NEW(readable => reading, writable => writing);
			Wasi.Cli.Stderr.RESULT_FUTURE_DROP_READABLE(
				future := Wasi.Cli.Stderr.WRITE_VIA_STREAM(data := reading));
			open := TRUE;
		END_IF;
		output(REQ := REQ, IN := IN, stream := writing);
		BUSY := output.BUSY;
		DONE := output.DONE;
		ERROR := output.ERROR;
	END_FUNCTION_BLOCK

	NAMESPACE Test
		USING Std.Unit;

		{test}
		FUNCTION_BLOCK test_stdout_takes_a_text
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				out: STDOUT;
			END_VAR
			out(REQ := TRUE, IN := 'Std.Console writes to stdout$N');
			ASSERT(NOT out.ERROR, 'STDOUT failed');
			DONE := out.DONE;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_stderr_takes_a_text
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				err: STDERR;
			END_VAR
			err(REQ := TRUE, IN := 'Std.Console writes to stderr$N');
			ASSERT(NOT err.ERROR, 'STDERR failed');
			DONE := err.DONE;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_an_empty_text_is_done_at_once
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				out: STDOUT;
			END_VAR
			out(REQ := TRUE, IN := '');
			ASSERT(out.DONE AND NOT out.BUSY AND NOT out.ERROR, 'an empty text was not done at once');
			DONE := TRUE;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_each_rising_edge_writes_once
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				out: STDOUT;
				step: INT;
			END_VAR
			IF step = 0 THEN
				// The first text, until the host has all of it.
				out(REQ := TRUE, IN := 'one, ');
				ASSERT(NOT out.ERROR, 'STDOUT failed');
				IF out.DONE THEN
					step := 1;
				END_IF;
			ELSIF step = 1 THEN
				// REQ held: no second write.
				out(REQ := TRUE, IN := 'not written');
				ASSERT(NOT out.DONE AND NOT out.BUSY, 'REQ held wrote again');
				out(REQ := FALSE, IN := '');
				step := 2;
			ELSE
				// A new edge writes the second text.
				out(REQ := TRUE, IN := 'two$N');
				ASSERT(NOT out.ERROR, 'STDOUT failed');
				DONE := out.DONE;
			END_IF;
		END_FUNCTION_BLOCK
	END_NAMESPACE
END_NAMESPACE
