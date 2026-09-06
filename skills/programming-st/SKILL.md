---
name: programming-st
description: Write IEC 61131-3 Structured Text for the `rk` compiler — POUs, VAR
  sections, types, literals, expressions, statements and pragmas. Use when
  creating or editing a .st file, or when ST syntax or a type name is uncertain.
---

## Summary

This is the syntax `rk` actually accepts, verified against the compiler and not against the IEC standard.
Keywords and identifiers are case-insensitive: `end_if` and `END_IF` are the same token, and `speed` and `Speed` are the same name.
A trailing `;` on a statement or a declaration is optional;
The canonical style is tabs, `name: TYPE` with no space before the colon, and variable sections indented inside their POU.
A workspace is a directory holding a `config.toml` and any number of `.st` files found recursively. `Std.*` resolves out of the box: the library is found beside the `rk` binary, or in the checkout's `stdlib/` when running from a build. `RK_STDLIB_PATH` overrides that (environment, or a `.env` at the workspace root); `rk env stdlib` prints the one in use, empty if there is none.

## POU kinds

Each POU is terminated by its own `END_` keyword, and every one of them lives at file top level (or inside a `NAMESPACE`, except `PROGRAM` and `CONFIGURATION`).

```iecst
FUNCTION Clamp: INT          // stateless; return type optional
	VAR_INPUT
		v: INT;
		lo: INT := 0;
		hi: INT := 100;
	END_VAR
	IF v < lo THEN
		Clamp := lo;         // the return value IS the function's own name
	ELSIF v > hi THEN
		Clamp := hi;
	ELSE
		Clamp := v;
	END_IF;
END_FUNCTION

FUNCTION_BLOCK Counter       // stateful; declare an instance, then call it
	VAR_INPUT
		CU: BOOL;
		PV: INT := 10;
	END_VAR
	VAR_OUTPUT
		Q: BOOL;
		CV: INT;
	END_VAR
	VAR
		prev: BOOL;
	END_VAR
	METHOD Reset             // methods go after the variables, before the body
		CV := 0;
	END_METHOD
	IF CU AND NOT prev THEN
		CV := CV + 1;
	END_IF;
	prev := CU;
	Q := CV >= PV;
END_FUNCTION_BLOCK

PROGRAM Main                 // top-level entry, bound to a task by a CONFIGURATION
	VAR RETAIN
		cycles: DINT;
	END_VAR
	cycles := cycles + 1;
END_PROGRAM

CLASS Motor IMPLEMENTS IMotor
	VAR
		running: BOOL;
	END_VAR
	METHOD PUBLIC Start: BOOL
		running := TRUE;
		Start := running;
	END_METHOD
END_CLASS

INTERFACE IMotor
	METHOD Start: BOOL
	END_METHOD
END_INTERFACE

TYPE                         // one TYPE block holds many declarations
	Level: INT(0..100);
END_TYPE
```

`FUNCTION` may declare a return type or not. `RETURN` takes no argument — `RETURN 1;` is a syntax error.
Access specifiers `PUBLIC` / `PRIVATE` / `PROTECTED` / `INTERNAL` go after the POU keyword (`FUNCTION PRIVATE Helper: INT`), after `METHOD`, and after `VAR`. Default is `PUBLIC`. See `programming-oop` for how they are enforced.
Order inside `FUNCTION_BLOCK` and `CLASS` is fixed: variable sections, then methods, then the body. A `METHOD` written after a statement is E0038.

## Variable sections

Every section is a `VAR…`/`END_VAR` pair. `VAR_INPUT`, `VAR_OUTPUT` and `VAR` take an optional qualifier on the same line: `VAR CONSTANT`, `VAR RETAIN`, `VAR NON_RETAIN`, `VAR_INPUT RETAIN`, `VAR_GLOBAL CONSTANT`.

| Section | FUNCTION | METHOD | FUNCTION_BLOCK | PROGRAM | CLASS | CONFIGURATION |
| --- | --- | --- | --- | --- | --- | --- |
| `VAR` (+ `CONSTANT`) | yes | yes | yes | yes | yes | no |
| `VAR_INPUT` | yes | yes | yes | yes | no | no |
| `VAR_OUTPUT` | yes | yes | yes | yes | no | no |
| `VAR_IN_OUT` | yes | yes | yes | yes | no (E0024) | no |
| `VAR_TEMP` | yes | yes | yes | yes | no (E0025) | no |
| `VAR_EXTERNAL` | yes | yes | yes | yes | yes | no |
| `VAR RETAIN` / `NON_RETAIN` | no | no | yes | yes | yes | no |
| `VAR_GLOBAL` | no | no | no | no | no | yes |

`VAR_GLOBAL` only exists inside a `CONFIGURATION`; anywhere else it is a syntax error. A POU reaches a global by declaring the same name in `VAR_EXTERNAL`; if no `CONFIGURATION` declares it, that is E0220.

`VAR_IN_OUT` passes by reference in both `FUNCTION` and `FUNCTION_BLOCK` — the callee writes through to the caller's variable.

Input defaults (`a: INT := 3;`) apply when the argument is omitted, in `FUNCTION`, `FUNCTION_BLOCK` and `PROGRAM` alike.

```iecst continues
CONFIGURATION Plant
	VAR_GLOBAL
		speed: INT := 5;
	END_VAR
	RESOURCE Cpu ON CPU
		TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
		PROGRAM p1 WITH Fast: Main;
	END_RESOURCE
END_CONFIGURATION
```

A workspace may declare exactly one `CONFIGURATION` (E0242). See `programming-config`.

## Overloading

Several `FUNCTION`s may share a name. The compiler picks one by the
ARGUMENTS; this is what replaced the removed `ANY_*` type classes, and it is
how the standard library declares `MAX`, `MIN`, `SEL`, `MUX` and the
conversion families.

```iecst
FUNCTION Scale : INT
	VAR_INPUT v : INT; END_VAR
	Scale := v * 2;
END_FUNCTION

FUNCTION Scale : REAL
	VAR_INPUT v : REAL; END_VAR
	Scale := v * 2.0;
END_FUNCTION

FUNCTION Use : INT
	VAR x : INT; y : REAL; END_VAR
	x := Scale(v := INT#3);        // the INT overload
	y := Scale(v := REAL#1.5);     // the REAL overload
	Use := x;
END_FUNCTION
```

Only `FUNCTION` overloads. Two `FUNCTION_BLOCK`s with one name are a
duplicate (E0101).

Overloads may differ by parameter type, by parameter COUNT, or by RETURN
type. Two that differ in nothing are a duplicate (E0101).

A set that differs only in its return type is resolved by the CONSUMING site
— the type the call is being assigned to picks the overload:

```iecst
FUNCTION f : INT
	VAR_INPUT a : INT; END_VAR
	f := 111;
END_FUNCTION
FUNCTION f : DINT
	VAR_INPUT a : INT; END_VAR
	f := 222;
END_FUNCTION

FUNCTION Use : INT
	VAR i : INT; d : DINT; END_VAR
	i := f(a := INT#1);        // i : INT  -> 111
	d := f(a := INT#1);        // d : DINT -> 222
	Use := i;
END_FUNCTION
```

A site with no expected type cannot pick, and is E0237 rather than a guess.

An argument that matches no overload exactly is widened, so a lone `DINT`
overload accepts an `INT` argument. If widening reaches more than one
candidate the call is ambiguous (E0237) and the compiler refuses to guess:

```iecst expect=E0237
FUNCTION h : DINT
	VAR_INPUT a : DINT; END_VAR
	h := 1;
END_FUNCTION
FUNCTION h : DINT
	VAR_INPUT a : LINT; END_VAR
	h := 2;
END_FUNCTION

FUNCTION Use : INT
	VAR d : DINT; END_VAR
	d := h(a := 5);            // E0237: 5 widens to DINT and to LINT alike
	d := h(a := DINT#5);       // fine: an exact match beats every widening
	Use := 0;
END_FUNCTION
```

A typed literal is the cheapest fix. When an exact match exists it always
wins, so `INT#5` and `DINT#5` each select their own overload even where both
are declared.

Two overloads must differ in something the CALL can see. Differing only by a
parameter NAME, or only by a `VAR_OUTPUT`, is a duplicate (E0101) — the
overload set is keyed on the input types, the input count and the return.

A default value does not blur an arity overload: given a 1-parameter and a
2-parameter version, `f(a := 1)` takes the 1-parameter one and
`f(a := 1, b := 2)` the other, even when the longer one could have defaulted
its second input.

**An overload set does not span namespaces.** Two same-named FUNCTIONs in
different namespaces are not an overload set — they are an ambiguity (E0225)
the moment both are imported, even when only one of them could possibly match
the arguments:

```iecst expect=E0225
NAMESPACE A
	FUNCTION f : INT
		VAR_INPUT a : INT; END_VAR
		f := 1;
	END_FUNCTION
END_NAMESPACE
NAMESPACE B
	FUNCTION f : REAL
		VAR_INPUT a : REAL; END_VAR
		f := 2.0;
	END_FUNCTION
END_NAMESPACE

FUNCTION Use : INT
	USING A;
	USING B;
	Use := f(a := INT#1);        // E0225, though only A.f takes an INT
	Use := A.f(a := INT#1);      // qualify instead
END_FUNCTION
```

Keep an overload set inside one namespace.

A workspace file that reopens a library namespace and redeclares one of its
overloads silently WINS — see the `programming-namespaces` skill; the same
silence applies here.

## Not implemented

SFC (`INITIAL_STEP` / `STEP` / `TRANSITION` / `ACTION`), ladder and FBD bodies parse but are discarded — the POU ends up with an empty body. Write ST bodies.

Direct variables (`%IX0.0`, `%QW4`, `AT %IX0.0`) are refused with E0245: the address is understood but nothing maps it to real I/O yet.

## Gotchas

Names are case-insensitive: `VAR i, I: INT;` is a duplicate (E0102), and a variable `p` shadows a type named `P`.

These short words are reserved and cannot be identifiers: `AT`, `BY`, `DO`, `TO`, `OF`, `REF`, `REF_TO`, `VAR`, `TYPE`, `CASE` — plus every other keyword. A variable named `by` or `at` fails with E0050, which reads as a confusing "unexpected token" on the *next* line.

`RETURN` never carries a value; assign to the function's own name instead.

Enum variants are always `Type#Variant`.

Bit tests need parentheses around the mask.

Sub-word types occupy 4 bytes of storage each (`BOOL`, `SINT`, `BYTE`, `CHAR`, `INT`, `WORD` are all 4-byte slots), but arithmetic and shifts still wrap at the declared IEC width (8 or 16 bits). Storage size and value width are not the same number.

`REAL_TO_INT(3.7)` is 4: real-to-integer rounds to nearest, ties to even, and saturates at the target's bounds. `TRUNC` is the one that drops the fraction.

Date and time comparisons are signed, matching the encodings: `T#-5s < T#0s`, and a pre-epoch `D#1969-12-31` orders before `D#1970-01-01`.

`Std.Math` has no `ADD`/`SUB`/`MUL`/`DIV`/`MOD` — those are operators. It has `ABS`, `SQRT`, `LN`, `LOG`, `EXP`, `EXPT`, the trig functions, and `IS_NAN`/`NOT_OK` to test a REAL before converting it. `Std.Convert` holds the `X_TO_Y` casts and `TRUNC`, `Std.Selection` has `SEL`/`MIN`/`MAX`/`LIMIT`/`MUX`, `Std.Bits` has `SHL`/`SHR`/`ROL`/`ROR`, `Std.Timers` has `TP_TIME`/`TON_TIME`/`TOF_TIME` (and `_LTIME` variants), `Std.Counters` `CTU`/`CTD`/`CTUD`, `Std.Edge` `R_TRIG`/`F_TRIG`, `Std.Bistable` `SR`/`RS`/`SEMA`, `Std.Memory` `MOVE`, `Std.Unit` `ASSERT`/`ASSERT_EQ`/`ASSERT_NEQ`.

## Reference files

Read the one that covers the question; each is self-contained.

- `references/types.md` — elementary types, literals, date/time encodings, `TYPE` declarations (STRUCT, ENUM, ARRAY, subrange, REF_TO)
- `references/syntax.md` — expressions and precedence, every statement form, call syntax, and a namespace sketch (the `programming-namespaces` skill covers scoping, `USING` and visibility in full)
- `references/pragmas.md` — `{test}`, `{extern}`, `{wasm}` and the extern contract
