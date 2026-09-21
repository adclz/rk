## The module

A workspace compiles to one core WebAssembly module.
It does not depend on the WASI Component model.

It imports its linear memory as `env.memory`, and nothing else unless the program uses the timers or declares an `{extern}` FUNCTION.
That holds for a release build. A debug build is not optimized, so it always imports the clock the stdlib timers read, `wasi:clocks/monotonic-clock@0.2.6` `now`.
It exports `__init`, which sets the cold-start values, one body per PROGRAM, and every FUNCTION marked `{export}`, under its qualified name.
A debug build also exports the workspace's `{test}` functions, for `rk test`.
Nothing else is exported: not a plain FUNCTION, not a FUNCTION_BLOCK body or a METHOD, not an `{extern}` import, and nothing of the stdlib.
A library's `{test}` functions are not compiled at all.
The retained and global bands are exported as `retain_base` / `retain_size` and `globals_base` / `globals_size`.

Nothing allocates and nothing calls `memory.grow`, so the footprint is settled at compile time.
Everything lives in the memory the host provided; the first 16 KiB are reserved for the grafted builtins.

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

A `FUNCTION` takes every parameter in the order it is declared:

- A `VAR_INPUT` scalar by value, a `STRING` input as `(ptr, len)`.
- One pointer per `VAR_IN_OUT` and `VAR_OUTPUT`; a `STRING` one is `(addr, capacity)`, so the callee's writes clamp.
- The return type is the result. A `STRING` result is `(ptr, len)`, an aggregate result is a pointer to the callee's slot to copy out of.

The order is the source order, not "inputs first": declaring `VAR_OUTPUT` before `VAR_INPUT` moves it up the parameter list.

```iecst
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
	total := total + raw * gain;
	clamped := total > REAL#100.0;
	Scale := total;
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

The host writes `total` and reads `clamped` back at the addresses it passed.

A namespaced POU is exported under its path, `Ns.Name`, and a FUNCTION_BLOCK body as `Name$__body__`.

## Imports

A host import declared with `{extern}` is the same convention in reverse: the `VAR_INPUT` are the parameters, the scalar `VAR_OUTPUT` are the results, and the return type comes last.
It takes copies, so `VAR_IN_OUT`, aggregate outputs and a `STRING` return are refused (`E1502`).
The `programming-st` skill's `references/pragmas.md` has the full contract.
