## Pragmas

Only the pragmas below are recognised. Anything else in braces is a syntax error, so `{attribute '…'}` and similar annotations from other toolchains must not be copied in.

`{test}` marks a `FUNCTION` as a test entry point for `rk test`. Not valid on other POU kinds.

`{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.

`{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU. Note the `=`.

`{extern 'module' 'name'}` declares the `FUNCTION` as a WASM import — see below.

`{wasm [type_ref] 'instruction' (params a b) (result r)}` is a statement: it emits one WASM instruction on the operands it names and stores the value in `r`, a parameter, a local or the FUNCTION's return (E0253 otherwise). A body may hold several, in order, with ordinary statements between them. `type_ref` names a variable whose type picks the instruction's numeric prefix. An instruction the compiler does not emit is E0248; a pragma outside a FUNCTION is E0249.

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

`{once}`, `{warn}` and `{info}` produce nothing unless `config.toml` contains a `[linter]` section — the linter is off without it. See `tool-linter`.

## The extern contract

`{extern}` sits **above** the `FUNCTION`, like `{test}`. It carries only the import's module and name; the declaration itself is the signature. `VAR_INPUT` become the parameters in declaration order, scalar `VAR_OUTPUT` become the results in declaration order, and the return type is the last result.

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

What is refused with E0243: `VAR_IN_OUT` ("an extern takes copies, not references"), any aggregate or `STRING` `VAR_OUTPUT` ("only scalar outputs cross an import"), and any statement in the body ("an extern FUNCTION has no statements"). Putting `{extern}` on anything other than a `FUNCTION` is E0244.

A `STRING` **return type** is refused the same way: "the return type of 'f' can only be a scalar".

### `{allow 'rule-name' ...}`

Silences the named lint rules at one site: above a POU or METHOD, the whole
POU; as a statement, the NEXT statement (nested bodies included). Names are
the `[linter.rules]` names; several fit one pragma; an unknown one is L0005
and silences nothing.

```iecst fragment
{allow 'missing-input-param'}
mb(REQ := TRUE, MODE := USINT#1); // deliberate partial re-call
```
