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
	   from WASI's WIT: each output's `WRITE_VIA_STREAM_WRITER`, in
	   `Wasi.Cli.Stdout` and in `Wasi.Cli.Stderr`, makes a stream of bytes,
	   hands its reading end to the host's `write-via-stream` and writes to
	   it. The error code the host answers with is not passed on: a write
	   only sees that the host let go. */

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
			output: Wasi.Cli.Stdout.WRITE_VIA_STREAM_WRITER;
		END_VAR

		output(REQ := REQ, IN := IN, CLOSE := FALSE);
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
			output: Wasi.Cli.Stderr.WRITE_VIA_STREAM_WRITER;
		END_VAR

		output(REQ := REQ, IN := IN, CLOSE := FALSE);
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
