+++
title = "Formatter"
description = "How rk fmt formats Structured Text, and what it deliberately leaves alone."

[extra]
lede = "One way to write it, so a diff shows what changed rather than who typed it."
md = "/formatter/index.md"
eyebrow = "tools"
+++
```sh
rk fmt              # rewrite every .st file in the workspace
rk fmt --check      # report what would change; exit 1 if anything would
```

In an editor it is the language server's *Format Document*, so the same rules apply whether a person or a script asks.

## What it guarantees

The formatter works on the parsed syntax tree, not on the text, so it cannot produce a file that no longer parses. It needs the file to parse going in, and refuses the whole file if it does not; semantic errors like a type mismatch or an unresolved name do not stop it. Its own suite formats twice and requires the second pass to change nothing, and continuous integration reformats two corpora on every change, the standard library and the grammar's own test fixtures, checking that no program's meaning moved.

## What it does not do

It has no line-width target and never reflows your expressions. A 300-character condition stays on one line if that is how you wrote it, and a call you split across lines stays split. What it normalises is spacing, indentation and the placement of declarations, which is the part people argue about in review.

## Indentation

One tab per level, and every block indents: variable sections, bodies, methods, namespaces, classes and `TYPE` blocks.

```iecst fragment
IF condition THEN
	x := 1;
	IF nested THEN
		y := 2;
	END_IF;
END_IF;
```

## Declarations

One declaration per line, so a name is never hidden behind a semicolon halfway across the line, and the type is separated by a single space after the colon.

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

Binary operators, assignments and comparisons get one space either side; the accessors get none.

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

## Lists: you choose the shape

A parameter list or an initialiser is written on one line or spread over several, and **a line break anywhere inside it is the instruction**. Keep it on one line and the formatter leaves it there; put a newline in it and every element gets its own line, with the closing bracket back at the statement's indent. Nothing depends on how long the line is, so the shape is yours to decide and the formatter only makes it consistent.

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

Initialisers follow the same rule, so a small struct stays inline and a large one opens up.

```iecst decl
p: Pt := (x := 1, y := 2);
q: Pt := (
	x := 1,
	y := 2
);
```

## Semicolons

The parser accepts a missing `;` at the end of a declaration or a statement, so a file that omits one still checks and still compiles. The formatter writes it in: every declaration, statement and directive comes back terminated, and one already there is left alone. A `USING` naming several namespaces takes one terminator at the end, not one per name.

## What it never touches

Comments and string literals are left exactly as written. A comment keeps its place, above the thing it describes or at the end of its line, and the spacing inside a string is yours.

```iecst fragment
VAR_INPUT
	(** this stays on top **)
	IN: BOOL; (* and this stays here *)
END_VAR
	s := '  spacing   inside   a   string  ';
```

## Blank lines

A blank line between declarations, POUs or variable sections is a paragraph break, so it is kept. Several in a row collapse to one, which is the only part of your vertical spacing the formatter has an opinion about.
