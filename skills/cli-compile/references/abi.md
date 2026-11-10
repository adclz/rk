## The module

A workspace compiles to one core WebAssembly module.
It does not depend on the WASI Component model.

It defines its own linear memory, and imports nothing unless the program reads a clock of `Std.Timers`, writes through `Std.Console` or declares an `{extern}` FUNCTION.
`Std.Console` writes to WASI 0.3's `wasi:cli/stdout` and `wasi:cli/stderr`, each a stream of bytes the module keeps open, with the component model's stream built-ins.
That holds for a release build. A debug build is not optimized, so it always imports the clock the stdlib timers read, `wasi:clocks/monotonic-clock@0.3.0` `now`.
It exports what a host calls:

- `__init`, which sets the cold-start values.
- One body per PROGRAM, `Type$__body__`.
- `Inst$__scan__` for a program instance with connections: it copies the inputs in, runs the body and copies the outputs out.
- The body of a FUNCTION_BLOCK a task runs on its own (`fb1 WITH Fast`), `FB$__body__`.
- Every FUNCTION marked `{export}`, as a function of the component typed in WIT (below).
- In a debug build, the workspace's `{test}` functions, for `rk test`.

Nothing else is exported: not a plain FUNCTION, not another FUNCTION_BLOCK body or a METHOD, not an `{extern}` import, and nothing of the stdlib.
A library's `{test}` functions are not compiled at all.

The memory is exported as `memory`, and each band as a `_base` and a `_size` global: `retain`, `globals`, and `input`, `output` and `marker` for an area the program uses.
The `programming-config` skill's `references/direct-variables.md` describes the areas.

Nothing allocates and nothing calls `memory.grow`, so the footprint is settled at compile time.
Everything lives in the module's memory; the first 16 KiB are reserved for the grafted builtins.

`__init` is only emitted when there is a constant initializer to write, so a host looks it up rather than assuming it.

## Types

| ST | at a call boundary | in memory |
| --- | --- | --- |
| `BOOL`, `SINT`…`DINT`, `USINT`…`UDINT`, `BYTE`, `WORD`, `DWORD`, `CHAR`, `TIME`, `DATE`, `TOD`, `DT` | `i32` | 4 bytes |
| `LINT`, `ULINT`, `LWORD`, `LTIME`, `LDATE`, `LTOD`, `LDT` | `i64` | 8 bytes |
| `REAL` / `LREAL` | `f32` / `f64` | 4 / 8 bytes |
| enum, subrange | the lane of its storage type | as its storage type |
| `STRING` | `(ptr, len)`, two `i32` | 4-byte length, then `capacity` bytes of UTF-8 |
| `STRUCT`, `ARRAY`, FB instance | `i32` pointer | laid out in place |
| `REF_TO T` | `i32` address; `NULL` is `0` | 4 bytes |

A sub-word type occupies a 4-byte slot, but its arithmetic still wraps at the declared IEC width.

## Calling an export

Call `__init` first, `() -> ()`.

A `PROGRAM` body takes its instance address and returns nothing: `(i32) -> ()`.

A `FUNCTION` marked `{export}` is a function of the component, typed in WIT from its declaration, in one interface named after the project (`Plant_A` is `plant-a`).
Its name there is its namespaces' names and its own in lowercase, each `_` written `-`, joined by `-` (`Motion.Scale_Axis` is `motion-scale-axis`), or the one `{export 'name'}` gives.
Each `VAR_INPUT` is a parameter and the return type the result: BOOL is `bool`, the integers and bit strings `s8` to `u64`, REAL and LREAL `f32` and `f64`, CHAR `char`, STRING `string`, a duration or a date the signed integer it is held in, an enum or a subrange the type it is stored as.

```iecst
{export}
FUNCTION Scale : REAL
	VAR_INPUT
		raw: INT;
		gain: REAL;
	END_VAR
	Scale := raw * gain;
END_FUNCTION
```

is, in the project `plant`:

```wit
export plant: interface {
    scale: func(raw: s16, gain: f32) -> result<f32, string>;
}
```

The result is what the FUNCTION returns, or the message of what stopped it: an exception, a `STRING` passed longer than its parameter, a value outside its parameter's subrange, a `STRING` returned that is not UTF-8.
None of them traps, so the host can call again.
`VAR_OUTPUT`, `VAR_IN_OUT`, `REF_TO`, STRUCT, ARRAY and instances are refused on an export (`E1509`).

A namespaced POU is exported under its path, `Ns.Name`, and a FUNCTION_BLOCK body as `Name$__body__`.

## Imports

A host import declared with `{extern 'module' 'name'}` takes its `VAR_INPUT` as the parameters and returns its return type.
Its module and name are written as WIT writes them (`'modbus' 'read-coils'`, `'acme:modbus/rtu@1.0.0'`), never converted (`E1512`).
An import from a module other than WASI's is typed in WIT from its declaration, in the `wit/` folder beside the component: scalars and `STRING` inputs only, 16 values at most, no `VAR_OUTPUT` and no `REF_TO` (`E1502`).
Each call checks a `STRING` for UTF-8 and a `CHAR` for a character before the host is called, and raises when one is not.
The `programming-st` skill's `references/pragmas.md` has the full contract.
