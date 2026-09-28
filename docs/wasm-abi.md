# WASM ABI

A workspace compiles to one core module:

```schema
core.wasm                           one module for the workspace
├─ IMPORT
│  ├─ env.memory                    the memory you provide
│  ├─ wasi:clocks now               the clock, when a timer runs, and always in debug
│  └─ {extern} FUNCTION             the host's, when you declare some
├─ EXPORT                           what a host calls
│  ├─ __init                        every POU's starting values, called first
│  ├─ Main$__body__                 a PROGRAM
│  ├─ C1$__scan__                   a program instance with connections
│  ├─ Filter$__body__               a function block a task runs
│  ├─ Scale                         a FUNCTION marked {export}
│  ├─ test_scale                    a {test}, in the debug profile only
│  ├─ memory                        the memory, exported back
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
| `BOOL`, `SINT`…`DINT`, `USINT`…`UDINT`, `BYTE`, `WORD`, `DWORD`, `CHAR`, `TIME`, `DATE`, `TOD`, `DT` | `i32` | 4 bytes |
| `LINT`, `ULINT`, `LWORD`, `LTIME`, `LDATE`, `LTOD`, `LDT` | `i64` | 8 bytes |
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
  
- Everything lives in the memory you provided; the first 16 KiB are reserved for the grafted builtins.

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

A host import declared with `{extern}` is the same convention in reverse: 
- `VAR_INPUT` are the parameters, 
- scalar `VAR_OUTPUT` the results, the return type last; 

it takes copies, so `VAR_IN_OUT`, aggregate outputs and a `STRING` return are refused.
