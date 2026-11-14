## Pragmas

Only the pragmas below are recognised.
Anything else in braces is a syntax error, so `{attribute '…'}` and similar annotations from other toolchains must not be copied in.

`{test}` marks a `FUNCTION` as a test entry point for `rk test`.
The runner calls it with no arguments, so it has no `VAR_INPUT`, `VAR_OUTPUT` or `VAR_IN_OUT` (E1511).
On a `FUNCTION_BLOCK` it marks a test that runs once per cycle, on an instance of its own, until a cycle leaves its `DONE` output TRUE: the block declares `DONE : BOOL` in `VAR_OUTPUT` (E1517), and no `VAR_INPUT` or `VAR_IN_OUT` (E1511).
Not valid on other POU kinds (E1503).

`{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.

`{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU.
Note the `=`.

`{export}` marks a `FUNCTION` a host calls, under its WIT name — see below. `{export 'name'}` gives the name.

`{extern 'module' 'name'}` declares the `FUNCTION` as a WASM import — see below.

`{wasm [type_ref] 'instruction' (params a b) (result r)}` is a statement: it emits one WASM instruction on the operands it names and stores the value in `r`, a parameter, a local or the FUNCTION's return (E1506 otherwise).
A body may hold several, in order, with ordinary statements between them.
`type_ref` names a variable whose type picks the instruction's numeric prefix.
An instruction the compiler does not emit is E1505; a pragma outside a FUNCTION is E1504; operands that do not fit the instruction's signature, lane for lane, are E1507, checked before anything reaches the module validator.

```iecst
USING Std.Unit;

FUNCTION Add : INT
	VAR_INPUT a : INT; b : INT; END_VAR
	Add := a + b;
END_FUNCTION

{test}
FUNCTION t_add
	ASSERT_EQ(Add(2, 3), 5, 'Add(2,3)');
END_FUNCTION

{warn = 'deprecated, use NewInit'}
FUNCTION OldInit: BOOL
	OldInit := TRUE;
END_FUNCTION

FUNCTION MyShl: WORD
	VAR_INPUT
		IN: WORD;
		N: INT;
	END_VAR
	{wasm IN 'shl' (params IN N) (result MyShl)}
END_FUNCTION

FUNCTION Round: DINT
	VAR_INPUT IN: REAL; END_VAR
	VAR r: REAL; END_VAR
	{wasm 'f32.nearest' (params IN) (result r)}
	{wasm 'i32.trunc_sat_f32_s' (params r) (result Round)}
END_FUNCTION
```

`{warn}`, `{info}` and `{once}` report with no configuration at all: `warn-pragma` and `once-violation` are recommended rules, and the linter does not need a `[linter]` section to be on.
See `tool-linter`.

## The export contract

`{export}` sits **above** the `FUNCTION`, like `{test}`.
The FUNCTION becomes a function a host calls, typed in WIT from its declaration: the `VAR_INPUT` are its parameters and the return type its result.
With `VAR_OUTPUT`, the result is a record named as the function is with `-outputs` after it (`measure-outputs`): each output in declaration order, then the return value, named as the function is.
The component exports them all in one interface named after the project (`Plant_A` is `plant-a`).

The function's name is the FUNCTION's namespaces' names and its own, in lowercase with each `_` written `-`, joined by `-`: `Motion.Scale_Axis` is `motion-scale-axis`.
`{export 'name'}` gives another, written as WIT writes one (`'read-coils'`).
E1513 refuses a name that gives no WIT name (`_Reset`, a letter outside ASCII), a given name that is not one, and two exports with one name; the component model compares names without their `-`, so `ab-c` and `a-bc` are one.

Nothing else a workspace declares is exported, apart from `__init`, the bodies the schedule runs (a PROGRAM's, a program instance's `Inst$__scan__`, a FUNCTION_BLOCK's a task runs on its own) and, in a debug build, the `{test}` functions.
An export is a root for the optimizer, so a release build drops every FUNCTION that is neither exported nor called, the standard library's included.

```iecst
{export}
FUNCTION Scale: REAL
	VAR_INPUT
		raw: INT;
		gain: REAL;
	END_VAR
	Scale := raw * gain;
END_FUNCTION

{export 'set-recipe'}
FUNCTION SetRecipe: BOOL
	VAR_INPUT
		name: STRING[32];
	END_VAR
	SetRecipe := name <> '';
END_FUNCTION
```

What may cross: scalar `VAR_INPUT` (BOOL, the integers and bit strings, REAL, LREAL, CHAR, the durations and dates, an enum or a subrange) and `STRING` inputs, 16 values at most with a STRING counting two, and scalar or `STRING` outputs and return type.
Each `VAR_OUTPUT` is named in WIT as a parameter is; E1513 refuses one named as the function is, the return value's field, and the build refuses a record named as another function of the interface.
The host gets a `result`: what the FUNCTION returned, or the message of what stopped it. That is an exception, a `STRING` passed longer than its parameter ("a STRING passed by the host is longer than its parameter"), a value outside its parameter's subrange, or a `STRING` returned that is not UTF-8. None of them traps.

Putting `{export}` on anything other than a `FUNCTION` is E1508: a PROGRAM is exported already, and a FUNCTION_BLOCK or a METHOD needs an instance the host does not have.

What is refused with E1509, because there is no single function to export: an `{extern}` FUNCTION (an import has no body), a `{test}` FUNCTION (exported for the runner already), a FUNCTION that takes an interface (compiled once per implementation), a variadic FUNCTION (compiled once per arity) and an overloaded FUNCTION (the name belongs to several).
Also E1509, because the host passes values and gets values back: a `VAR_IN_OUT`, a `REF_TO`, STRUCT, ARRAY or instance input, output or return type, and a 17th value.
Export a plain FUNCTION that calls it instead.

## The extern contract

`{extern}` sits **above** the `FUNCTION`, like `{test}`.
It carries only the import's module and name; the declaration itself is the signature: `VAR_INPUT` become the parameters in declaration order, and the return type is the result.
With `VAR_OUTPUT`, the result is the record of the outputs and the return value, named as an export's are (`read-register-outputs`, its last field `read-register`).

The module and the name are written as WIT writes them, never converted (E1512):
the module is a WIT name (`'modbus'`) or an interface (`'acme:modbus/rtu@1.0.0'`, one namespace and one interface, no nested name), and the name is a WIT name, lowercase words and digits joined by `-` (`'read-coils'`, not `'read_coils'`).

```iecst
{extern 'wasi:clocks/monotonic-clock@0.3.0' 'now'}
FUNCTION MONOTONIC_LTIME: LTIME
END_FUNCTION

{extern 'modbus' 'read-coils'}
FUNCTION ReadCoils: UDINT
	VAR_INPUT
		start_address: UINT;   // `start-address` in WIT
		count: UINT;
	END_VAR
END_FUNCTION
```

An import from a module other than WASI's is typed in WIT from its declaration, in the `wit/` folder `rk compile` writes beside the component.
What may cross: a scalar `VAR_INPUT` (BOOL, the integers and bit strings, REAL, LREAL, CHAR, the durations and dates, an enum or a subrange), a `STRING` input as WIT's `string`, and scalar outputs and return type, 16 inputs at most with a STRING counting two.
Each `VAR_INPUT` and `VAR_OUTPUT` is named in WIT by its name in lowercase, each `_` written `-`: a name with an empty word (`x_`, `a__b`), or a letter outside ASCII, is E1512, and so are two names that differ only by a `_` (`ab`, `a_b`) and an output named as the import is.

Each call checks what it passes before the host is called: a `STRING` that is not UTF-8 raises "a STRING passed to the host is not UTF-8", and a `CHAR` that is a surrogate or past U+10FFFF raises "a CHAR passed to the host is not a character". The component model would trap on either, stopping the PLC; the exception is caught by `__TRY` like any other.

What is refused with E1502 on every import: `VAR_IN_OUT` ("an extern takes copies, not references"), a STRUCT, ARRAY or instance `VAR_INPUT` (on a WASI import, "no WASI function reads rk's own layout"), an `ARRAY[*]` or variadic `VAR_INPUT`, an aggregate or `STRING` `VAR_OUTPUT`, a `STRING` or aggregate return type ("the return type of 'f' can only be a scalar"), and any statement in the body ("an extern FUNCTION has no statements").
On an import from a module other than WASI's, also: a `REF_TO` input ("an address in the module means nothing to the host") and a 17th value.
Putting `{extern}` on anything other than a `FUNCTION` is E1501.

A WASI import keeps the types WASI's WIT gives it, in their core form: a record of scalars comes back in `VAR_OUTPUT`, one for each field in order, as `Std.Timers` reads the system clock, and a `REF_TO` input is an address where a record can come back too.

A function WIT declares an `async func` is imported by a FUNCTION_BLOCK, with `{extern async 'module' 'name'}`; `{extern async}` on a FUNCTION, and `{extern}` without `async` on a FUNCTION_BLOCK, are E1501.
Each call of the block takes the call one step: it starts the call when none is running and checks it otherwise, never waiting.
The block declares `DONE : BOOL` in VAR_OUTPUT (E1514), TRUE in the call that saw the call finish; DONE is the block's own and no part of the signature, its other VAR_OUTPUTs are the result's fields, and its VAR_INPUTs the parameters, read when the call starts.
While the call runs, a call of the block leaves its VAR_INPUTs as they are: it evaluates what it passes, and drops it.
Its instance lives in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL, never in a FUNCTION's or METHOD's variables or a VAR_TEMP (E1515): the host writes the result into it at a later scan. A FUNCTION can still call one it is handed as VAR_IN_OUT, or a global one through VAR_EXTERNAL.
The block is its call: statements, a METHOD, an EXTENDS or an IMPLEMENTS on it are E1502.
REQ and BUSY, where a block's users need them, are written in ST by a block that calls it.
The build checks a WASI import against WASI's WIT, and names the declaration when it does not fit: the interface has the function, `{extern async}` names an `async func`, and `{extern}` a `func`.

A WASI import may be a resource's function, named as WIT names it: `[constructor]R`, `[method]R.f` (its first parameter the handle) or `[static]R.f`; a handle is a `UDINT`, and `[resource-drop]R` is an `{extern}` FUNCTION taking the one handle it drops (E1516 otherwise). A host module's import cannot name one (E1512): only WASI's interfaces have resources.

A WASI import may also be a stream's or a future's built-in, named after the function whose type has the stream (`[stream-new-0]write-via-stream`, or `[stream-drop-readable-0][method]descriptor.read-via-stream` for a method); elsewhere that name is E1512.
The number counts the function's streams and futures from 0, its parameters first, and the build checks that it names one of the built-in's kind.
`new` and the drops return at once and are `{extern}` FUNCTIONs; `new` declares the two ends it makes as two `UDINT` VAR_OUTPUTs, the reading end first, and no return type.
A stream's write, `[stream-write-N]f`, waits for the reader and is an `{extern async}` FUNCTION_BLOCK taking the writing end (`UDINT`) and the bytes (`STRING`, unchecked) and giving, after DONE, how many went (`UDINT`) and whether the reader left (`BOOL`).
The host reads the bytes from the block while the copy runs: a call made then checks the copy and leaves the bytes as they are, so each call may pass the same arguments.
A stream's read, `[stream-read-N]f`, is an `{extern async}` FUNCTION_BLOCK taking the reading end (`UDINT`) and giving, after DONE, the bytes in a `STRING` (its length the count) or an `ARRAY OF BYTE`, how many came (`UDINT`) and whether the writer left (`BOOL`); the bytes are whole in the call where DONE is TRUE.
A future's read, `[future-read-N]f`, takes the future's reading end and gives, after DONE, its value's fields; a future's write is not imported yet. A built-in that does not fit its POU, or other variables, is E1516.

```iecst
{extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
FUNCTION NEW_STREAM
	VAR_OUTPUT
		readable: UDINT;
		writable: UDINT;
	END_VAR
END_FUNCTION
```

### `{allow 'rule-name' ...}`

Silences the named lint rules at one site: above a POU or METHOD, the whole POU; as a statement, the NEXT statement (nested bodies included).
Names are the `[linter.rules]` names; several fit one pragma; an unknown one is L0005 and silences nothing.

```iecst fragment
{allow 'missing-input-param'}
mb(REQ := TRUE, MODE := USINT#1); // deliberate partial re-call
```
