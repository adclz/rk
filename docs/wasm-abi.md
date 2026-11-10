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

- A `FUNCTION` marked `{export}` takes every parameter in the order it is declared: 
  - `VAR_INPUT` scalars by value.
  - One pointer per `VAR_IN_OUT` and `VAR_OUTPUT`.
  
  - the return type is the result, a `STRING` result is `(ptr, len)`, an aggregate result is a pointer to the callee's slot to copy out of.
  
- Everything lives in the module's memory, which it exports as `memory`; the first 16 KiB are reserved for the grafted builtins.

So this function:

```st
{export}
FUNCTION Scale : REAL
	VAR_INPUT
		raw: INT;
		gain: REAL;
	END_VAR
	VAR_IN_OUT
		total: REAL;
	END_VAR
	VAR_OUTPUT
		clamped: BOOL;
	END_VAR

END_FUNCTION
```

exports this:

```wat
(func (export "Scale")
	(param i32)   ;; raw      INT, by value
	(param f32)   ;; gain     REAL, by value
	(param i32)   ;; total    VAR_IN_OUT, address
	(param i32)   ;; clamped  VAR_OUTPUT, address
	(result f32)) ;; Scale    REAL
```

Write `total` and read `clamped` back at the addresses you passed.
`clamped` starts over at every call, so what you left at its address is overwritten.
Declaring `VAR_OUTPUT` before `VAR_INPUT` moves it up the parameter list.

A host function declared with `{extern 'module' 'name'}` is an import.
Its module and its name are written as WIT writes them, and rk converts neither (`E1512`):

- the module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`;
- the name is a WIT name, lowercase words and digits joined by `-`, like `read-coils`.

An import from a module other than WASI's is typed in WIT from its declaration, in the `wit/` folder `rk compile` writes beside the component.
A module named by a WIT name is an interface the world declares itself, and one named by an interface's name is a package under `wit/deps/`.
Each `VAR_INPUT` is a parameter, named by its name in lowercase with each `_` written `-`, and the return type is the result:

| ST | WIT |
|---|---|
| `BOOL` | `bool` |
| `SINT`, `INT`, `DINT`, `LINT` | `s8`, `s16`, `s32`, `s64` |
| `USINT`, `UINT`, `UDINT`, `ULINT` | `u8`, `u16`, `u32`, `u64` |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | `u8`, `u16`, `u32`, `u64` |
| `REAL`, `LREAL` | `f32`, `f64` |
| `CHAR` | `char` |
| `STRING`, as an input | `string` |
| `TIME`, `DATE`, `TOD` | `s32` |
| `LTIME`, `LDATE`, `LTOD`, `DT`, `LDT` | `s64` |
| enum, subrange | as the type it is stored as |

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

The component model passes values, not addresses, and a WIT function returns one value.
So such an import has no `VAR_IN_OUT`, `VAR_OUTPUT`, `REF_TO`, STRUCT, ARRAY or instance, and takes 16 values at most, a `STRING` counting two (`E1502`).

The host reads a `STRING` as text, so it has to be UTF-8, and a `CHAR` has to be a Unicode character.
Each call checks what it passes before the host is called:

- a `STRING` that is not UTF-8 raises `a STRING passed to the host is not UTF-8`;
- a `CHAR` that is a surrogate, or past U+10FFFF, raises `a CHAR passed to the host is not a character`.

A `__TRY` catches them like any exception, see [Exceptions](exceptions.md).
Unchecked, the component model would trap on them, and a trap stops the whole PLC.

In the core module, a `STRING` input is `(ptr, len)` and a scalar is its lane, as in the first table.

A WASI import keeps the types WASI's WIT gives it, in their core form: `VAR_INPUT` are the parameters and the return type the result.
A `REF_TO` input is an address, which is how a record comes back: `Std.Timers` declares WASI's system clock this way, whose `now` writes its `instant` where the module points, and the zone's `utc-offset`, whose `option<s64>` comes back the same way.

The clocks a module imports are WASI 0.3's, in their core forms, and the host provides them:

| Import | Signature | What it does |
|---|---|---|
| `wasi:clocks/monotonic-clock@0.3.0` `now` | `() -> i64` | nanoseconds of a clock that never goes back |
| `wasi:clocks/system-clock@0.3.0` `now` | `(i32)` | writes at the address the seconds since 1970-01-01 UTC as an `s64`, negative before, then the nanoseconds as a `u32` |
| `wasi:clocks/timezone@0.3.0` `utc-offset` | `(i64, i32, i32)` | takes an instant as its seconds and nanoseconds, and writes at the address whether the host has a zone, in one byte, and if it has, 8 bytes on, the nanoseconds the zone is east of UTC at that instant |

A release build imports the ones the program reads; a debug build imports all three, so a host that has no zone still defines `utc-offset`, answering that it has none, or the module does not instantiate. Where the monotonic clock starts is the host's choice, as WASI leaves it.
