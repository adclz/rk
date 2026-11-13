# WASM ABI

A workspace compiles to one component, named after the project: `rk compile` writes `rk_build/debug/plant.wasm` for the project `plant`, and `rk_build/release/plant.wasm` with `--release`.
The WIT that types it is written in `wit/` beside it: `world.wit`, and each package the world names under `deps/`.

The component imports the WASI interfaces the program uses, and the host's own, from its `{extern}` FUNCTIONs.
It exports the workspace's `{export}` FUNCTIONs, in an interface named after the project, and the control interface every component has, `rk:plc/control@0.1.0`, through which a host runs the PLC:

```wit
interface control {
    init: func() -> result<_, string>;
    run-task: func(task: u32, inputs: list<u8>) -> result<list<u8>, scan-error>;
    save-retain: func() -> list<u8>;
    restore-retain: func(retained: list<u8>);
    read-memory: func(ranges: list<range>) -> result<list<u8>, memory-error>;
    write-memory: func(address: u32, bytes: list<u8>) -> result<_, memory-error>;
    run-test: func(test: u32) -> result<test-state, test-error>;

    variant scan-error { raised(string), no-such-task }
    record range { address: u32, size: u32 }
    variant memory-error { outside-memory(u32), too-large, outside-variables }
    variant test-error { failed(string), no-such-test }
    enum test-state { passed, running }
}
```

- `init` writes the starting value of every variable. A host calls it once, before anything else. Its error is the message of an exception a starting value raised.
- `save-retain` gives the retain area, to keep across a power cycle, and `restore-retain` gives it back after `init` on a warm start.
- `run-task` runs the task at that place in the `rk.schedule` section: `inputs` is copied into the input area, each program instance of the task runs, and the output area comes back. A host calls it for each task the section says is due, in the order the section lists them. The `located-map` section says where each address is in the two lists.
- `read-memory` reads ranges of the memory between two scans, and `write-memory` forces bytes into the variables, at the addresses the `debug-symbols` section gives. Writing anywhere but the variables is refused.
- `run-test` runs the test at that place in the `test-manifest` section: a test FUNCTION whole, `passed` when it returns, or one cycle of a test FUNCTION_BLOCK, `running` until a cycle leaves its `DONE` TRUE. The block keeps its instance from one call to the next, and a host calls it again for its next cycle. A release build has no test.

No function of the interface waits.
An exception never leaves the component, where it would be a trap that stops the whole PLC: it comes back as the error of the result, `raised` with its message for a scan.

The WIT of the interface, with each case explained, is `wit/deps/rk-plc/control.wit`.

Inside the component, one core module does the work:

```schema
core module                         one for the workspace
├─ IMPORT
│  ├─ wasi:clocks now, utc-offset   the clocks a timer or Std.Timers reads, and always in debug
│  └─ {extern} FUNCTION             the host's, when you declare some
├─ EXPORT                           what the component calls
│  ├─ __init                        every POU's starting values, called first
│  ├─ Main$__body__                 a PROGRAM
│  ├─ C1$__scan__                   a program instance with connections
│  ├─ Filter$__body__               a function block a task runs
│  ├─ Scale                         a FUNCTION marked {export}
│  ├─ test_scale                    a {test}, in the debug profile only
│  ├─ test_valve$__test__           a {test} FUNCTION_BLOCK, a cycle per call
│  ├─ memory                        the module's own memory
│  └─ input_base, input_size        each band, see Memory bands
├─ INTERNAL                         what only the module calls
│  ├─ Motor$__body__                a function block a program calls
│  └─ Std.Math.SIN                  the stdlib, only what you call
└─ CUSTOM SECTIONS                  what a host reads
   ├─ rk.schedule                   the tasks
   ├─ retain-map                    what persists across a power cycle
   ├─ located-map                   where each address lives
   └─ debug-symbols                 names, types and places
```

Everything the component calls is exported, everything else stays internal, the stdlib included: a release build only keeps what you call.

`__init` writes the starting value of every variable, in every POU.
Call it once, before anything else, then restore the retained values over it.

Each band is exported as a `_base` and a `_size`, see [Memory bands](memory-bands.md).

| ST | at a call boundary | in memory |
|---|---|---|
| `BOOL`, `SINT`…`DINT`, `USINT`…`UDINT`, `BYTE`, `WORD`, `DWORD`, `CHAR`, `TIME`, `DATE`, `TOD` | `i32` | 4 bytes |
| `LINT`, `ULINT`, `LWORD`, `LTIME`, `LDATE`, `LTOD`, `DT`, `LDT` | `i64` | 8 bytes |
| `REAL` / `LREAL` | `f32` / `f64` | 4 / 8 bytes |
| enum, subrange | the lane of its storage type | as its storage type |
| `STRING` | `(ptr, len)`, two `i32` | 4-byte length, then `capacity` bytes of UTF-8 |
| `STRUCT`, `ARRAY`, FB instance | `i32` pointer | laid out in place |
| `REF_TO T` | `i32` address; `NULL` is `0` | 4 bytes |

Calling an export:

- `__init` first, `() -> ()`.
 
- A `PROGRAM` body takes its instance address and does not have a return type: `(i32) -> ()`.

- Everything lives in the module's memory, which it exports as `memory`; the first 16 KiB are reserved for the grafted builtins.

A `FUNCTION` marked `{export}` is a function a host calls, typed in WIT from its declaration.
The component exports them all in one interface named after the project: the project `Plant_A` exports `plant-a`.

A FUNCTION's name there is its namespaces' names and its own, in lowercase with each `_` written `-`, joined by `-`: `Motion.Scale_Axis` is `motion-scale-axis`.
`{export 'name'}` gives it another name, and two exports cannot have one (`E1513`).

Each `VAR_INPUT` is a parameter, named the same way, and the return type is the result:

| ST | WIT |
|---|---|
| `BOOL` | `bool` |
| `SINT`, `INT`, `DINT`, `LINT` | `s8`, `s16`, `s32`, `s64` |
| `USINT`, `UINT`, `UDINT`, `ULINT` | `u8`, `u16`, `u32`, `u64` |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | `u8`, `u16`, `u32`, `u64` |
| `REAL`, `LREAL` | `f32`, `f64` |
| `CHAR` | `char` |
| `STRING` | `string` |
| `TIME`, `DATE`, `TOD` | `s32` |
| `LTIME`, `LDATE`, `LTOD`, `DT`, `LDT` | `s64` |
| enum, subrange | as the type it is stored as |

So this function, in the project `plant`:

```st
{export}
FUNCTION Scale : REAL
	VAR_INPUT
		raw: INT;
		gain: REAL;
	END_VAR
	Scale := raw * gain;
END_FUNCTION
```

is this export:

```wit
export plant: interface {
    scale: func(raw: s16, gain: f32) -> result<f32, string>;
}
```

A FUNCTION with `VAR_OUTPUT` returns a record, named as the function is with `-outputs` after it: each output, named as a parameter is, in the order they are declared, then the return value, named as the function is.
So this function:

```st
{export}
FUNCTION Measure : REAL
	VAR_INPUT
		raw: INT;
	END_VAR
	VAR_OUTPUT
		unit: STRING[8];
		clamped: BOOL;
	END_VAR
	unit := 'mbar';
	clamped := raw > 1000;
	Measure := raw;
END_FUNCTION
```

is this export:

```wit
export plant: interface {
    record measure-outputs {
        unit: string,
        clamped: bool,
        measure: f32,
    }
    measure: func(raw: s16) -> result<measure-outputs, string>;
}
```

A `VAR_OUTPUT` named as the function is would be a second field of that name (`E1513`), and a record named as another function of the interface would be a second item of that name, which the build refuses.

The result is what the FUNCTION returns, or the message of the exception that stopped it.
An exception never leaves the component, where it would be a trap, and a trap stops the whole PLC.
What a host passes is checked the same way, before the FUNCTION runs:

- a `STRING` longer than its parameter is the error `a STRING passed by the host is longer than its parameter`;
- a value outside its parameter's subrange is the error `value out of subrange bounds`.

And so is what it gets back: a `STRING` that is not UTF-8 and a `CHAR` that is no character are the errors an import raises, below.

The component model passes values, not addresses.
So an export has no `VAR_IN_OUT`, `REF_TO`, STRUCT, ARRAY or instance, and takes 16 values at most, a `STRING` counting two (`E1509`).

A host function declared with `{extern 'module' 'name'}` is an import.
Its module and its name are written as WIT writes them, and rk converts neither (`E1512`):

- the module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`;
- the name is a WIT name, lowercase words and digits joined by `-`, like `read-coils`.

An import from a module other than WASI's is typed in WIT from its declaration, in the `wit/` folder `rk compile` writes beside the component.
A module named by a WIT name is an interface the world declares itself, and one named by an interface's name is a package under `wit/deps/`.
Each `VAR_INPUT` is a parameter, and the return type is the result, or the record of the `VAR_OUTPUT` and the return type, named and typed as an export's are, except that an import returns no `STRING`.

So this declaration:

```st
{extern 'modbus' 'read-coils'}
FUNCTION ReadCoils : UDINT
	VAR_INPUT
		start_address: UINT;
		count: UINT;
	END_VAR
END_FUNCTION
```

is this import:

```wit
import modbus: interface {
    read-coils: func(start-address: u16, count: u16) -> u32;
}
```

Like an export, such an import has no `VAR_IN_OUT`, `REF_TO`, STRUCT, ARRAY or instance, and takes 16 values at most, a `STRING` counting two (`E1502`).

The host reads a `STRING` as text, so it has to be UTF-8, and a `CHAR` has to be a Unicode character.
Each call checks what it passes before the host is called:

- a `STRING` that is not UTF-8 raises `a STRING passed to the host is not UTF-8`;
- a `CHAR` that is a surrogate, or past U+10FFFF, raises `a CHAR passed to the host is not a character`.

A `__TRY` catches them like any exception, see [Exceptions](exceptions.md).
Unchecked, the component model would trap on them, and a trap stops the whole PLC.

In the core module, a `STRING` input is `(ptr, len)` and a scalar is its lane, as in the first table.
A record of one field is that field.
A record of two fields or more is written by the host at an address the call passes last: each field at the next multiple of its size, a `BOOL` and an 8-bit integer in one byte, a 16-bit one in two.

A WASI import keeps the types WASI's WIT gives it, in their core form: `VAR_INPUT` are the parameters, and the `VAR_OUTPUT` and the return type the result, as above.
A record of scalars comes back in `VAR_OUTPUT`, one for each of its fields in their order.
`Std.Timers` declares WASI's system clock this way, `now` with `seconds: LINT` and `nanoseconds: UDINT`, and the zone's `utc-offset` with `known: BOOL` and `offset: LINT`: its `option<s64>` is laid out as a record of a byte that says whether the value is there, then the value.
A `REF_TO` input is an address, where a record can come back too.
A STRUCT, an ARRAY or an instance input is refused (`E1502`): it would go as an address into rk's own layout, which no WASI function reads.

A host function WIT declares an `async func` is imported by a FUNCTION_BLOCK, with `{extern async 'module' 'name'}`.
A scan never waits for it: each call of the block takes the call one step, starting it when none is running and checking it otherwise.

```st
{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
FUNCTION_BLOCK WAIT_FOR
	VAR_INPUT
		how_long: LTIME;
	END_VAR
	VAR_OUTPUT
		DONE: BOOL;
	END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
	VAR
		wait: WAIT_FOR;
		ticks: DINT;
	END_VAR
	wait(how_long := LT#100ms);
	IF wait.DONE THEN
		ticks := ticks + 1;
	END_IF;
END_PROGRAM
```

- The block's `VAR_INPUT` are the call's parameters, read when the call starts. While the call runs, a call of the block leaves them as they are: it evaluates what it passes, and drops it.
- Its `DONE : BOOL` is its own and no part of the function's signature (`E1514`). It is TRUE in the call of the block that saw the call finish, and FALSE in every other.
- Its other `VAR_OUTPUT` are the fields of the result, written when the call finishes, as an `{extern}` FUNCTION's are.
- A block that is not called leaves its call running, and the next call of the block sees it finish before it starts another.
- The block is its call: it has no statements, METHOD, EXTENDS or IMPLEMENTS (`E1502`).

The host writes the result into the block's instance at a later scan, so the instance lives in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL, never in a FUNCTION's or a METHOD's variables or a VAR_TEMP (`E1515`).
A FUNCTION can still call one it does not hold: the caller's, handed over as VAR_IN_OUT, or a global's, through VAR_EXTERNAL.
REQ, BUSY or any other control a block's users need is written in ST, by a block that calls it.
A call that has started cannot be cancelled: the component model's cancel needs a flag wasmtime has off by default.

The declaration says the function is async, and the build checks it against WASI's WIT: `{extern async}` names an `async func`, and a plain `{extern}` a `func`, whose call a scan cannot wait for.
The build also checks that WASI's interface has the function, and names the declaration when it does not.
For a host's own module, the WIT rk writes declares an `async func`, its result the record of the block's `VAR_OUTPUT` but `DONE`.

In the core module, the block's call is the import lowered async, `[async-lower]wait-for`: the parameters, or past 4 values the address of a tuple of them, then the address the result is written at, and a word back, the call's state and handle.
The block checks the call with the component model's built-ins, imported from `$root`: `[waitable-set-new]`, `[waitable-join]`, `[waitable-set-poll]`, which never waits, and `[subtask-drop]`.
A host that implements an async function asynchronously runs the component through wasmtime's async API, `instantiate_async` and `call_async`; the control interface's functions stay synchronous.

A WASI import may also be a stream's or a future's built-in, under the name of the function whose type has the stream: `[stream-new-0]write-via-stream` makes the stream of `write-via-stream`'s type.
The number counts the streams and futures of the function's type from 0, its parameters first, as wit-component counts them, and the build checks that it names one of the built-in's kind.
A built-in that returns at once, a stream's or a future's `new` or one of their drops, is an `{extern}` FUNCTION.
A `new` declares the two ends it makes, a `UDINT` `VAR_OUTPUT` each, the reading end first, and no return type (`E1516`).
A stream's write waits for the reader, so it is an `{extern async}` FUNCTION_BLOCK, `[stream-write-N]f`: it takes the stream's writing end, a `UDINT`, and the bytes, a `STRING` whose length is passed as it is, unchecked, and after `DONE` gives how many went, a `UDINT`, and whether the reader left, a `BOOL` (`E1516` otherwise).
Its call is a copy into the stream, which ends when the reader has taken some of the bytes or left: the block waits on the stream's end as it would on a call.
The host reads the bytes from the block while the copy runs: a call made then checks the copy, and leaves the bytes as they are, whatever it passes.

The clocks a module imports are WASI 0.3's, in their core forms, and the host provides them:

| Import | Signature | What it does |
|---|---|---|
| `wasi:clocks/monotonic-clock@0.3.0` `now` | `() -> i64` | nanoseconds of a clock that never goes back |
| `wasi:clocks/system-clock@0.3.0` `now` | `(i32)` | writes at the address the seconds since 1970-01-01 UTC as an `s64`, negative before, then the nanoseconds as a `u32` |
| `wasi:clocks/timezone@0.3.0` `utc-offset` | `(i64, i32, i32)` | takes an instant as its seconds and nanoseconds, and writes at the address whether the host has a zone, in one byte, and if it has, 8 bytes on, the nanoseconds the zone is east of UTC at that instant |

The time zone is still unstable in WASI 0.3, `@unstable(feature = clocks-timezone)`: a host has it only with that feature on, and its signature can change in a 0.3 release.
A release build imports the ones the program reads; a debug build imports all three, so a host that has no zone still defines `utc-offset`, answering that it has none, or the module does not instantiate. Where the monotonic clock starts is the host's choice, as WASI leaves it.

[`Std.Console`](console.md) writes to WASI 0.3's standard output and error, `wasi:cli/stdout@0.3.0` and `wasi:cli/stderr@0.3.0`, with the imports it declares.
Each is one function, `write-via-stream`, which takes a stream of bytes.
A block makes its stream at its first request, hands the reading end to the host, keeps the other end, and writes to it with the component model's stream built-ins:

| Import, beside `write-via-stream` | Signature | What it does |
|---|---|---|
| `[stream-new-0]write-via-stream` | `() -> i64` | a stream of bytes: its reading end in the low half, its writing end in the high half, which the call writes into the FUNCTION's two outputs |
| `write-via-stream` | `(i32) -> i32` | hands the reading end to the host, which answers with a future the module gives up |
| `[future-drop-readable-1]write-via-stream` | `(i32)` | gives that future up |
| `[async-lower][stream-write-0]write-via-stream` | `(i32, i32, i32) -> i32` | hands `len` bytes at `ptr` to the writing end, and returns at once |

A component's host provides all four by providing WASI's command line interfaces, as wasmtime-wasi does.
A scan never waits for a write: one the host has not taken stays pending in the block, `BUSY`, and a later scan sees it end.
The host's side of the stream runs while the host calls the component through wasmtime's async API; a synchronous call hands over only what the host takes within it.

Both outputs are written with the one block `Std.Console` imports beside the standard output's function, so a program that writes to either imports the standard output's interface; a debug build imports both.
