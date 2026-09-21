## Pragmas

Only the pragmas below are recognised.
Anything else in braces is a syntax error, so `{attribute '…'}` and similar annotations from other toolchains must not be copied in.

`{test}` marks a `FUNCTION` as a test entry point for `rk test`.
Not valid on other POU kinds.

`{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.

`{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU.
Note the `=`.

`{export}` marks a `FUNCTION` as a WASM export, under its name — see below.

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

`{warn}` and `{info}` report with no configuration at all: `warn-pragma` is one of the recommended rules, and the linter does not need a `[linter]` section to be on.
`{once}` is the exception, because `once-violation` is opt-in: it reports only under `select = "all"` or an explicit `[linter.rules]` entry.
See `tool-linter`.

## The export contract

`{export}` sits **above** the `FUNCTION`, like `{test}`, and takes no argument.
The FUNCTION is exported under its name, qualified by its namespace (`Plant.Reset`), and the declaration is the signature: the `cli-compile` skill's ABI reference gives what each parameter becomes.

Nothing else a workspace declares is exported, apart from `__init`, the PROGRAM bodies the schedule names and, in a debug build, the `{test}` functions.
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
```

Putting `{export}` on anything other than a `FUNCTION` is E1508: a PROGRAM is exported already, and a FUNCTION_BLOCK or a METHOD needs an instance the host does not have.

What is refused with E1509, because there is no single function to export: an `{extern}` FUNCTION (an import has no body), a `{test}` FUNCTION (exported for the runner already), a FUNCTION that takes an interface (compiled once per implementation), a variadic FUNCTION (compiled once per arity) and an overloaded FUNCTION (the name belongs to several).
Export a plain FUNCTION that calls it instead.

## The extern contract

`{extern}` sits **above** the `FUNCTION`, like `{test}`.
It carries only the import's module and name; the declaration itself is the signature.
`VAR_INPUT` become the parameters in declaration order, scalar `VAR_OUTPUT` become the results in declaration order, and the return type is the last result.

```iecst
{extern 'wasi:clocks/monotonic-clock@0.2.6' 'now'}
FUNCTION MONOTONIC_LTIME: LTIME
END_FUNCTION

{extern 'rt' 'sample'}
FUNCTION Sample: INT
	VAR_INPUT
		channel: INT;
		label: STRING;
	END_VAR
	VAR_OUTPUT
		value: LINT;
		status: INT;
	END_VAR
END_FUNCTION
```

What may cross: scalar `VAR_INPUT` by value, a `STRING` input as a `(ptr, len)` pair, a struct or array input as a pointer to a call-entry copy, scalar `VAR_OUTPUT` as results, and a scalar return type as the last result.

What is refused with E1502: `VAR_IN_OUT` ("an extern takes copies, not references"), any aggregate or `STRING` `VAR_OUTPUT` ("only scalar outputs cross an import"), and any statement in the body ("an extern FUNCTION has no statements").
Putting `{extern}` on anything other than a `FUNCTION` is E1501.

A `STRING` **return type** is refused the same way: "the return type of 'f' can only be a scalar".

### `{allow 'rule-name' ...}`

Silences the named lint rules at one site: above a POU or METHOD, the whole POU; as a statement, the NEXT statement (nested bodies included).
Names are the `[linter.rules]` names; several fit one pragma; an unknown one is L0005 and silences nothing.

```iecst fragment
{allow 'missing-input-param'}
mb(REQ := TRUE, MODE := USINT#1); // deliberate partial re-call
```
