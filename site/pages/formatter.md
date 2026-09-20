+++
title = "Formatter"
description = "How rk fmt formats Structured Text"

[extra]
lede = "How rk fmt formats Structured Text"
md = "/formatter/index.md"
+++
```sh
rk fmt              # rewrite every .st file in the workspace
rk fmt --check      # report what would change; exit 1 if anything would
```

Rk uses [Topiary 🌳](https://topiary.tweag.io/) to format files.

## Indentation

One tab per level, and every block indents:

Variable sections, bodies, methods, namespaces, classes and `TYPE` blocks.

```iecst fragment
IF condition THEN
	x := 1;
	IF nested THEN
		y := 2;
	END_IF;
END_IF;
```

## Declarations

One declaration per line, with a single space after the colon.

```iecst fragment
// as written
VAR a : INT; b : INT; c : INT; END_VAR

// as formatted
VAR
	a: INT;
	b: INT;
	c: INT;
END_VAR
```

## Spacing

Binary operators, assignments and comparisons get one space on each side.
Accessors get none.

```iecst fragment
// as written
m_iCurrentValue:= m_iCurrentValue+1;
x :=a>b AND c<>d;
ok := Color # Red;
v := arr [ 0 ];

// as formatted
m_iCurrentValue := m_iCurrentValue + 1;
x := a > b AND c <> d;
ok := Color#Red;
v := arr[0];
```

## Lists

A parameter list or an initializer can be written on one line, or spread over several lines.

**A line break anywhere inside the list is the instruction:**

- Keep it on one line, and the formatter leaves it on one line.
- Put a newline in it, and every element gets its own line, with the closing bracket back at the indentation of the statement.

> [!NOTE]
> Nothing depends on how long the line is, so the shape is up to you, the formatter only makes it consistent.

```iecst fragment
// written on one line, so it stays on one line
n := Sum(a := 1, b := 2, c := 3);

// written with a break after the first argument…
n := Sum(a := 1,
	b := 2, c := 3);

// …so all of them get their own line
n := Sum(
	a := 1,
	b := 2,
	c := 3
);
```

Initializers follow the same rule, so a small struct stays inline and a large one opens up.

```iecst decl
p: Pt := (x := 1, y := 2);
q: Pt := (
	x := 1,
	y := 2
);
```

## Semicolons

Semicolons `;` are not mandatory, see [Syntax](/#syntax).

The formatter writes the missing ones in: every declaration, statement and directive comes back terminated, and one already there is left alone.

> [!NOTE]
> A `USING` naming several namespaces takes one terminator at the end, not one per name.

## What it never touches

Comments and string literals are left exactly as written.

- A comment keeps its place, above the thing it describes or at the end of its line.
- The spacing inside a string is yours.

```iecst fragment
VAR_INPUT
	(** this stays on top **)
	IN: BOOL; (* and this stays here *)
END_VAR
	s := '  spacing   inside   a   string  ';
```

## Blank lines

A blank line between declarations, POUs or variable sections is kept.
Several blank lines in a row collapse to one.

This is the only opinion the formatter has about your vertical spacing.
