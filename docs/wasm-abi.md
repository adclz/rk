# WASM ABI

A workspace compiles to one core module:

```schema
core.wasm                           one module for the workspace
├─ IMPORT
│  ├─ wasi:clocks now, utc-offset   the clocks a timer or Std.Timers reads, and always in debug
│  └─ {extern} FUNCTION             the host's, when you declare some
├─ EXPORT                           what a host calls
│  ├─ __init                        every POU's starting values, called first
│  ├─ Main$__body__                 a PROGRAM
│  ├─ C1$__scan__                   a program instance with connections
│  ├─ Filter$__body__               a function block a task runs
│  ├─ Scale                         a FUNCTION marked {export}
│  ├─ test_scale                    a {test}, in the debug profile only
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

Everything a host calls is exported, everything else stays internal, the stdlib included: a release build only keeps what you call.

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
A `REF_TO` input is an address, which is how a record comes back: `Std.Timers` declares WASI's system clock this way, whose `now` writes its `instant` where the module points, and the zone's `utc-offset`, whose `option<s64>` comes back the same way.
A record of scalars can come back in `VAR_OUTPUT` too, one for each of its fields in their order: `now` with `seconds: LINT` and `nanoseconds: UDINT`.

The clocks a module imports are WASI 0.3's, in their core forms, and the host provides them:

| Import | Signature | What it does |
|---|---|---|
| `wasi:clocks/monotonic-clock@0.3.0` `now` | `() -> i64` | nanoseconds of a clock that never goes back |
| `wasi:clocks/system-clock@0.3.0` `now` | `(i32)` | writes at the address the seconds since 1970-01-01 UTC as an `s64`, negative before, then the nanoseconds as a `u32` |
| `wasi:clocks/timezone@0.3.0` `utc-offset` | `(i64, i32, i32)` | takes an instant as its seconds and nanoseconds, and writes at the address whether the host has a zone, in one byte, and if it has, 8 bytes on, the nanoseconds the zone is east of UTC at that instant |

A release build imports the ones the program reads; a debug build imports all three, so a host that has no zone still defines `utc-offset`, answering that it has none, or the module does not instantiate. Where the monotonic clock starts is the host's choice, as WASI leaves it.

[`Std.Console`](console.md) writes to WASI 0.3's standard output and error, `wasi:cli/stdout@0.3.0` and `wasi:cli/stderr@0.3.0`.
Each is one function, `write-via-stream`, which takes a stream of bytes.
The module makes the stream at its first write, hands its reading end to the host, keeps the other end, and writes to it with the component model's stream built-ins:

| Import, beside `write-via-stream` | Signature | What it does |
|---|---|---|
| `[stream-new-0]write-via-stream` | `() -> i64` | a stream of bytes: its reading end in the low half, its writing end in the high half |
| `write-via-stream` | `(i32) -> i32` | hands the reading end to the host, which answers with a future the module gives up |
| `[future-drop-readable-1]write-via-stream` | `(i32)` | gives that future up |
| `[async-lower][stream-write-0]write-via-stream` | `(i32, i32, i32) -> i32` | hands `len` bytes at `ptr` to the writing end, and returns at once |

A component's host provides all four by providing WASI's command line interfaces, as wasmtime-wasi does.
A function a host calls may not wait, so a write is never waited for: a host that takes none of the bytes leaves them pending in the module's memory, and the output closed.

A release build imports an output when the program writes to it, and a debug build imports both.
