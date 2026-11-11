# Console

`Std.Console` writes text to the host's standard output and standard error, with a block for each.

```st
USING Std.Console;

PROGRAM Station
	VAR
		started: BOOL;
		log: STDOUT;
	END_VAR
	log(REQ := started, IN := 'pump station started$N');
END_PROGRAM
```

| Block | Writes to |
|---|---|
| `STDOUT` | the standard output |
| `STDERR` | the standard error |

A rising edge of `REQ` writes `IN` once, as it holds it, with no line break added: end the text with `$N`.
`IN` holds 4096 bytes, and a longer `STRING` is cut there, as any input is.

| Output | TRUE |
|---|---|
| `BUSY` | while the host takes the text |
| `DONE` | in the call where the host has all of it, or at once for an empty text |
| `ERROR` | when the host has let go of its end: the output stays closed, and each later request is an error at once |

A rising edge while `BUSY` is ignored: check `BUSY` before asking again.

## A scan does not wait

A block hands the text to the host and returns.
A host that is ready takes it within the call.
What it does not take there, it takes at later calls, while `BUSY` stays TRUE.

Each block writes through a stream of bytes of its own, made at its first request.
Two blocks hand the host two streams: each text comes out in its order, but a text the host takes in parts can have the other block's text land between them.

## In a test

A test FUNCTION holds no block (`E1515`): it writes through blocks declared in a `VAR_GLOBAL`, which a host that is ready takes within the call.
`rk test` keeps what a test writes, and shows it under the test when it fails, see [Tests](tests.md).

## For a host

The two outputs are WASI's, see [WASM ABI](wasm-abi.md).
A host calls the component through wasmtime's async API, as `rk test` does: a synchronous call hands over only what the host takes within it, and a text it leaves for later stays `BUSY`.
