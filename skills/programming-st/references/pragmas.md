## Pragmas

Only the pragmas below are recognised.
Anything else in braces is a syntax error, so `{attribute '…'}` and similar annotations from other toolchains must not be copied in.

`{test}` marks a `FUNCTION` as a test entry point for `rk test`.
The runner calls it with no arguments, so it has no `VAR_INPUT`, `VAR_OUTPUT` or `VAR_IN_OUT` (E1511).
Not valid on other POU kinds.

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

What may cross: scalar `VAR_INPUT` (BOOL, the integers and bit strings, REAL, LREAL, CHAR, the durations and dates, an enum or a subrange) and `STRING` inputs, 16 values at most with a STRING counting two, and a scalar or `STRING` return type.
The host gets a `result`: what the FUNCTION returned, or the message of what stopped it. That is an exception, a `STRING` passed longer than its parameter ("a STRING passed by the host is longer than its parameter"), a value outside its parameter's subrange, or a `STRING` returned that is not UTF-8. None of them traps.

Putting `{export}` on anything other than a `FUNCTION` is E1508: a PROGRAM is exported already, and a FUNCTION_BLOCK or a METHOD needs an instance the host does not have.

What is refused with E1509, because there is no single function to export: an `{extern}` FUNCTION (an import has no body), a `{test}` FUNCTION (exported for the runner already), a FUNCTION that takes an interface (compiled once per implementation), a variadic FUNCTION (compiled once per arity) and an overloaded FUNCTION (the name belongs to several).
Also E1509, because the host passes values and gets one back: a `VAR_OUTPUT`, a `VAR_IN_OUT`, a `REF_TO`, STRUCT, ARRAY or instance input, a return type that is one of those, and a 17th value.
Export a plain FUNCTION that calls it instead.

## The extern contract

`{extern}` sits **above** the `FUNCTION`, like `{test}`.
It carries only the import's module and name; the declaration itself is the signature: `VAR_INPUT` become the parameters in declaration order, and the return type is the result.

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
What may cross: a scalar `VAR_INPUT` (BOOL, the integers and bit strings, REAL, LREAL, CHAR, the durations and dates, an enum or a subrange), a `STRING` input as WIT's `string`, and a scalar return type, 16 values at most with a STRING counting two.
Each `VAR_INPUT` is named in WIT by its name in lowercase, each `_` written `-`: a name with an empty word (`x_`, `a__b`), or a letter outside ASCII, is E1512, and so are two names that differ only by a `_` (`ab`, `a_b`).

Each call checks what it passes before the host is called: a `STRING` that is not UTF-8 raises "a STRING passed to the host is not UTF-8", and a `CHAR` that is a surrogate or past U+10FFFF raises "a CHAR passed to the host is not a character". The component model would trap on either, stopping the PLC; the exception is caught by `__TRY` like any other.

What is refused with E1502 on every import: `VAR_IN_OUT` ("an extern takes copies, not references"), an `ARRAY[*]` or variadic `VAR_INPUT`, an aggregate or `STRING` `VAR_OUTPUT`, a `STRING` or aggregate return type ("the return type of 'f' can only be a scalar"), and any statement in the body ("an extern FUNCTION has no statements").
On an import from a module other than WASI's, also: any `VAR_OUTPUT` ("a WIT function returns one value"), a `REF_TO` input ("an address in the module means nothing to the host"), a STRUCT, ARRAY or instance input, and a 17th value.
Putting `{extern}` on anything other than a `FUNCTION` is E1501.

A WASI import keeps the types WASI's WIT gives it, in their core form: a `REF_TO` input is the address where a record comes back, as `Std.Timers` reads the system clock.

### `{allow 'rule-name' ...}`

Silences the named lint rules at one site: above a POU or METHOD, the whole POU; as a statement, the NEXT statement (nested bodies included).
Names are the `[linter.rules]` names; several fit one pragma; an unknown one is L0005 and silences nothing.

```iecst fragment
{allow 'missing-input-param'}
mb(REQ := TRUE, MODE := USINT#1); // deliberate partial re-call
```
