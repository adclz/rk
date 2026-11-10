# Console

`Std.Console` writes text to the host's standard output and standard error.

```st
USING Std.Console;

PROGRAM Station
	VAR
		announced: BOOL;
		written: BOOL;
	END_VAR
	IF NOT announced THEN
		written := WRITE_STDOUT('pump station started$N');
		announced := TRUE;
	END_IF;
END_PROGRAM
```

| Function | Writes to |
|---|---|
| `WRITE_STDOUT(IN)` | the standard output |
| `WRITE_STDERR(IN)` | the standard error |

Both write the `STRING` as it holds it, and add no line break: end the text with `$N`.
`IN` holds 4096 bytes, and a longer `STRING` is cut there, as any input is.

They return `TRUE` when the host has taken all of the text.

## A scan does not wait

A write hands the text to the host and returns.
A host that is ready takes it within the call: `rk test` does, and so does a host that gives the module its own standard output.

A host that takes none of the text leaves that output closed.
The write returns `FALSE`, and so does every later write to it, for as long as the program runs.
The other output goes on.

## In a test

`rk test` keeps what a test writes, and shows it under the test when it fails, see [Tests](tests.md).

## For a host

The two outputs are WASI's, see [WASM ABI](wasm-abi.md).
