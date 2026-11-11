/*
The host's standard output and standard error.

`WRITE_STDOUT` and `WRITE_STDERR` write a `STRING` as it holds it, with no
line break added, and return `TRUE` when the host has taken all of it.
End the text with `$N` for a line break.

Both write to WASI's
[standard output and error](https://github.com/WebAssembly/WASI/blob/main/proposals/cli/README.md),
each a stream of bytes the module keeps for as long as it runs.
`IN` holds 4096 bytes, and a longer `STRING` is cut there, as any input is.

A scan never waits for the host. A host that is ready takes the text
within the call. One that takes none of it leaves the output closed: the
write returns `FALSE`, and so does every later one to that output.
*/
NAMESPACE Std.Console

	/* Writes `IN` to the host's standard output, and returns `TRUE` when the
	   host has taken all of it, `FALSE` when it took a part or none, or has
	   closed its output.

	   # Examples

	   ```iecst
	   VAR
	       written: BOOL;
	   END_VAR

	   written := WRITE_STDOUT('pump started$N');
	   ```
	*/
	FUNCTION WRITE_STDOUT : BOOL
		VAR_INPUT
			IN : STRING[4096];
		END_VAR
		{wasm 'rk.stdout_write' (params IN) (result WRITE_STDOUT)}
	END_FUNCTION

	/* Writes `IN` to the host's standard error, and returns `TRUE` when the
	   host has taken all of it, `FALSE` when it took a part or none, or has
	   closed its error output.

	   # Examples

	   ```iecst
	   VAR
	       written: BOOL;
	   END_VAR

	   written := WRITE_STDERR('pressure out of range$N');
	   ```
	*/
	FUNCTION WRITE_STDERR : BOOL
		VAR_INPUT
			IN : STRING[4096];
		END_VAR
		{wasm 'rk.stderr_write' (params IN) (result WRITE_STDERR)}
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit;

		{test}
		FUNCTION test_write_stdout_is_taken
			ASSERT(WRITE_STDOUT('Std.Console writes to stdout$N'), 'WRITE_STDOUT was not taken');
		END_FUNCTION

		{test}
		FUNCTION test_write_stderr_is_taken
			ASSERT(WRITE_STDERR('Std.Console writes to stderr$N'), 'WRITE_STDERR was not taken');
		END_FUNCTION

		{test}
		FUNCTION test_an_empty_write_is_taken
			ASSERT(WRITE_STDOUT(''), 'an empty write was not taken');
		END_FUNCTION

		{test}
		FUNCTION test_two_writes_go_through_one_stream
			ASSERT(WRITE_STDOUT('one, '), 'the first write was not taken');
			ASSERT(WRITE_STDOUT('two$N'), 'the second write was not taken');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
