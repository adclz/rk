---
name: programming-namespaces
description: Organise ST code with NAMESPACE, qualified names, USING directives
  and the visibility specifiers. Use when structuring a library, when a name
  does not resolve or resolves ambiguously, or when deciding what to expose.
---

## Summary

A `NAMESPACE` groups POUs and types under a dotted name.
Everything inside is reachable from outside by qualifying it; a `USING` directive makes it reachable unqualified.

```iecst
NAMESPACE App.Motors
	USING Std.Convert;              // directives first, inside the namespace

	FUNCTION Clamp : INT
		VAR_INPUT
			v : INT;
		END_VAR
		Clamp := v;
	END_FUNCTION
END_NAMESPACE

USING App.Motors;                   // file level

FUNCTION UseIt : INT
	UseIt := App.Motors.Clamp(v := 1);   // qualified: always works
	UseIt := Clamp(v := 1);              // unqualified: thanks to USING
END_FUNCTION
```

Inside a namespace, siblings call each other unqualified with no `USING` — the enclosing namespace is already in scope.

## Declaring

The same namespace may be opened more than once, in one file or across files; the fragments are one namespace.
This is how a library splits over files without a "partial" keyword.

```iecst
NAMESPACE App
	FUNCTION A : INT
		A := 1;
	END_FUNCTION
END_NAMESPACE

NAMESPACE App                        // same namespace, not a duplicate
	FUNCTION B : INT
		B := 2;
	END_FUNCTION
END_NAMESPACE
```

Namespaces nest, and a nested one is named through its parents (`A.B.f()`); `USING A.B;` imports the inner one directly.

`PROGRAM` and `CONFIGURATION` may NOT appear inside a namespace — E0024 and E0025 respectively.
They are the application's entry points, not library contents.

## USING

A `USING` may sit at file level or at the top of a POU header, before the variable sections:

```iecst
NAMESPACE App.Motors
	FUNCTION Clamp : INT
		VAR_INPUT v : INT; END_VAR
		Clamp := v;
	END_FUNCTION
END_NAMESPACE

FUNCTION UseIt : INT
	USING App.Motors;
	UseIt := Clamp(v := 1);
END_FUNCTION
```

One directive may name several namespaces, separated by commas, and the terminator goes at the end:

```iecst
NAMESPACE A
	FUNCTION f : INT
		f := 1;
	END_FUNCTION
END_NAMESPACE
NAMESPACE B
	FUNCTION g : INT
		g := 2;
	END_FUNCTION
END_NAMESPACE

FUNCTION UseIt : INT
	USING A, B;                      // one directive, two namespaces
	UseIt := f() + g();
END_FUNCTION
```

Naming a namespace that does not exist is E0204; repeating the same import is E0111.

When two imported namespaces both offer a name, the reference is ambiguous — E0205 — and the fix is to qualify it rather than to drop an import:

```iecst
NAMESPACE A
	FUNCTION f : INT
		f := 1;
	END_FUNCTION
END_NAMESPACE
NAMESPACE B
	FUNCTION f : INT
		f := 2;
	END_FUNCTION
END_NAMESPACE

FUNCTION UseIt : INT
	USING A;
	USING B;
	UseIt := A.f();                  // not `f()`: both A and B declare one
END_FUNCTION
```

## Visibility

`PUBLIC`, `PRIVATE`, `INTERNAL`, `PROTECTED` and `FINAL` mark methods and variables.
Note one deliberate deviation from the standard: an item that names no specifier is **PUBLIC** here, where IEC makes it PROTECTED — a hiding default would make `inst.Method()` an error in essentially all OOP ST.

- `PRIVATE` — only inside the defining POU (E1001 outside).
- `INTERNAL` — only inside the same namespace (E1003 from another).
- `PROTECTED` — the defining POU and its derivations.

```iecst expect=E1003
NAMESPACE A
	FUNCTION_BLOCK fb
		METHOD INTERNAL m : INT
			m := 1;
		END_METHOD
	END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE B
	FUNCTION use : INT
		VAR i : A.fb; END_VAR
		use := i.m();                // E1003: INTERNAL, and B is not A
	END_FUNCTION
END_NAMESPACE
```

**A FUNCTION takes one specifier: `PRIVATE`.** `FUNCTION PRIVATE helper` is callable only from POUs declared in its own namespace — nested namespaces included — and only on its own side of the library line: workspace code that reopens a library's namespace still cannot call the library's private functions (`E1005`).
Written at global scope it restricts nothing.
`PUBLIC` spells the default; `PROTECTED` and `INTERNAL` do not apply to a FUNCTION (`E1006`) This is an extension: the standard gives a FUNCTION no specifier at all.
`FUNCTION_BLOCK` and `CLASS` headers take none either; keep a whole one out of reach by not exporting it from a namespace anyone imports.

```iecst sketch
NAMESPACE Std.Mqtt
	{extern 'acme:mq@1' 'state'}
	FUNCTION PRIVATE MQ_STATE : DINT      // the shim behind a public FB
		VAR_INPUT session : DINT; END_VAR
	END_FUNCTION
END_NAMESPACE
```

## Relative paths and INTERNAL namespaces

A path written inside a namespace is resolved RELATIVE first: `Impl.hidden()` inside `NAMESPACE Lib` means `Lib.Impl.hidden()`, at any depth, and only then a top-level `Impl`.
The nearest match wins.
This holds for calls, types (`x : Impl.T`) and `USING Impl;` alike; the full path always works too.

`NAMESPACE INTERNAL Impl` is the standard's module-level privacy: everything in it is reachable only from inside the namespace that encloses it — nested namespaces included — on its own side of the library line.
From anywhere else, a qualified access, a type, or a `USING` of it is `E1004` (a `USING` is refused once, at the `USING`).
At the top level the enclosing namespace is the root, so `NAMESPACE INTERNAL` there means "this library only": workspace code cannot reach it, the library's own files can.

```iecst expect=E1004
NAMESPACE Lib
	NAMESPACE INTERNAL Impl
		FUNCTION Frame : INT                       // Lib's own business
			Frame := 1;
		END_FUNCTION
	END_NAMESPACE
	FUNCTION Send : INT
		Send := Impl.Frame();                    // relative, and allowed
	END_FUNCTION
END_NAMESPACE

FUNCTION Use : INT
	Use := Lib.Impl.Frame();                     // E1004
END_FUNCTION
```

`FUNCTION PRIVATE` (above) hides one function; `NAMESPACE INTERNAL` hides a whole group.
Both draw the same line.

## Reopening a library namespace

A workspace file may reopen `Std.Convert` (or any library namespace) and add to it.
Declaring a POU the library ALREADY defines is `E0102`, reported on the workspace declaration with the library's as the related span:

```iecst expect=E0102
NAMESPACE Std.Convert                // the stdlib already has this one
	FUNCTION LINT_TO_DINT : DINT     // E0102: duplicate POU
		VAR_INPUT IN : LINT; END_VAR
		LINT_TO_DINT := 999;
	END_FUNCTION
END_NAMESPACE
```

The error lands on the workspace side whichever file the compiler reads first — the library is never the one told to rename.
A duplicate between two WORKSPACE files is the same `E0102`.

A LOCAL declaration still wins over a name reached through `USING`, silently and by design: that is shadowing an import, not redefining a namespace's member.

## Error codes

- `E0024` / `E0025`: a PROGRAM / a CONFIGURATION inside a NAMESPACE
- `E0111`: the same USING twice
- `E0204`: USING names a namespace that does not exist
- `E0205`: the name is offered by more than one namespace in scope — qualify it
- `E1001` / `E1003`: PRIVATE / INTERNAL access refused

## Idioms

The standard library is the worked example: one namespace per protocol or domain (`Std.Modbus`, `Std.Mqtt`, `Std.Strings`), its own `USING` lines at the top, and a nested `Test` namespace holding the `{test}` functions:

```iecst sketch
NAMESPACE Std.Mqtt
	USING Std.Convert;

	FUNCTION_BLOCK MQTT_CONNECT
		…
	END_FUNCTION_BLOCK

	NAMESPACE Test
		USING Std.Unit;
		USING Std.Mqtt;

		{test}
		FUNCTION test_connects
			…
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
```

The nested `Test` namespace keeps test POUs from colliding with library names and out of a consumer's unqualified scope, while `rk test` still discovers them.

A namespace name and a POU name do not collide, so `NAMESPACE A` alongside a top-level `FUNCTION A` checks clean — but it reads badly.
Prefer distinct names.
