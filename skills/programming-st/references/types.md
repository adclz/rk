## Elementary types

`BOOL`, `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `LREAL`, `TIME`, `LTIME`, `DATE`, `LDATE`, `TIME_OF_DAY` (`TOD`), `LTIME_OF_DAY` (`LTOD`), `DATE_AND_TIME` (`DT`), `LDATE_AND_TIME` (`LDT`), `STRING`, `STRING[n]`, `CHAR`.

There is no `WSTRING`.
A bare `STRING` has a capacity of 80 bytes.

Widening is implicit exactly where the standard's table allows it, which is not "within a family": `INT` → `REAL` is implicit and `DINT` → `REAL` is not, and an unsigned type widens to a larger signed one (`USINT` → `INT`).
The two tables below are written by the site generator from the compiler's own rules.

<!-- casts:begin -->

<details>
<summary><strong>Implicit</strong>, what an assignment or a call widens on its own.</summary>

| from | to |
|---|---|
| `BOOL` | `BYTE`, `WORD`, `DWORD`, `LWORD` |
| `BYTE` | `WORD`, `DWORD`, `LWORD` |
| `WORD` | `DWORD`, `LWORD` |
| `DWORD` | `LWORD` |
| `LWORD` | — |
| `SINT` | `INT`, `DINT`, `LINT`, `REAL`, `LREAL` |
| `INT` | `DINT`, `LINT`, `REAL`, `LREAL` |
| `DINT` | `LINT`, `LREAL` |
| `LINT` | — |
| `USINT` | `INT`, `DINT`, `LINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `LREAL` |
| `UINT` | `DINT`, `LINT`, `UDINT`, `ULINT`, `REAL`, `LREAL` |
| `UDINT` | `LINT`, `ULINT`, `LREAL` |
| `ULINT` | — |
| `REAL` | `LREAL` |
| `LREAL` | — |
| `CHAR` | — |
| `STRING` | — |
| `TIME` | `LTIME` |
| `LTIME` | — |
| `DATE` | `LDATE` |
| `LDATE` | — |
| `TOD` | `LTOD` |
| `LTOD` | — |
| `DT` | `LDT` |
| `LDT` | — |

</details>

<details>
<summary><strong>Explicit</strong>, the <code>Std.Convert</code> functions, each named <code>FROM_TO_TO</code>.</summary>

| from | to |
|---|---|
| `BOOL` | `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `BYTE` | `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `CHAR`, `STRING` |
| `WORD` | `BYTE`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `DWORD` | `BYTE`, `WORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING` |
| `LWORD` | `BYTE`, `WORD`, `DWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `LREAL`, `STRING` |
| `SINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `INT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `DINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING`, `TIME`, `DATE`, `TOD` |
| `LINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `LREAL`, `STRING`, `LTIME`, `LDATE`, `LTOD`, `DT`, `LDT` |
| `USINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `STRING` |
| `UINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `USINT`, `STRING` |
| `UDINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `USINT`, `UINT`, `REAL`, `STRING` |
| `ULINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `REAL`, `LREAL`, `STRING` |
| `REAL` | `DWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `LREAL` | `LWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING` |
| `CHAR` | `BYTE`, `STRING` |
| `STRING` | — |
| `TIME` | `DINT`, `STRING`, `LTIME` |
| `LTIME` | `LINT`, `STRING`, `TIME` |
| `DATE` | `DINT`, `STRING`, `LDATE` |
| `LDATE` | `LINT`, `STRING`, `DATE` |
| `TOD` | `DINT`, `STRING`, `LTOD` |
| `LTOD` | `LINT`, `STRING`, `TOD` |
| `DT` | `LINT`, `STRING`, `DATE`, `LDATE`, `TOD`, `LTOD`, `LDT` |
| `LDT` | `LINT`, `STRING`, `DATE`, `LDATE`, `TOD`, `LTOD`, `DT` |

</details>

<!-- casts:end -->

Everything else — narrowing, signed to unsigned, integer/bit-string, real → integer — needs an explicit `Std.Convert` call, and E0301 names the one to use: `INT_TO_SINT(i)`, `REAL_TO_INT(r)`, `INT_TO_WORD(i)`.

Real → integer rounds to nearest, ties to even (`REAL_TO_INT(2.5)` is `2`, `REAL_TO_INT(3.5)` is `4`); `TRUNC` drops the fraction instead.
A value outside the target's range saturates to the nearest bound and NaN converts to `0`; `Std.Math` has `IS_NAN` and `NOT_OK` (NaN or infinite) to test a value before converting it.

## Literals

```iecst decl
i: INT := 42;               // untyped, inferred from the target
j: INT := INT#42;           // typed prefix
k: DINT := 1_000_000;       // underscores are separators
w: WORD := WORD#16#FF;      // 16# hex, 8# octal, 2# binary
m: BYTE := 2#1010_0101;
r: REAL := 3.14;
e: LREAL := LREAL#1.0e-3;
b: BOOL := TRUE;            // TRUE / FALSE, or BOOL#1 / BOOL#0
t: TIME := T#10ms;          // T# or TIME#; d h m s ms us ns
u: TIME := TIME#1d2h3m4s5ms;
l: LTIME := LT#500us;       // LT# or LTIME#
d: DATE := D#2024-01-31;    // D# or DATE#
o: TOD := TOD#12:30:00;
n: DT := DT#2024-01-31-12:30:00;
s: STRING := 'hello';       // single quotes
c: CHAR := 'A';             // untyped too, inferred from the target
h: CHAR := CHAR#'A';        // typed prefix, same value
```

Inside a single-quoted string, `$` escapes: `$$`, `$'`, `$L`, `$N`, `$P`, `$R`, `$T`, and `$41` for one hex byte.
Double-quoted strings are also accepted and produce a `STRING`.

A literal's characters are their UTF-8 bytes, which is how the runtime, the debugger and the network read a `STRING`: `'café'` is 5 bytes, `LEN` counts bytes, and a `STRING[4]` refuses it (E0314).
`FIND`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE` and `REPLACE` count bytes too and can cut through a character; their `CHAR_` twins in `Std.Strings` (`CHAR_COUNT`, `CHAR_AT`, `CHAR_FIND`, `CHAR_LEFT`, `CHAR_RIGHT`, `CHAR_MID`, `CHAR_INSERT`, `CHAR_DELETE`, `CHAR_REPLACE`) count characters, and `CHAR_AT` is the way from a `STRING` to a `CHAR`.

A quoted literal carries no type of its own, exactly like a bare number: it is a `STRING` where a `STRING` is expected and a `CHAR` where a `CHAR` is.
So `c: CHAR := 'A'` and `s: STRING := 'A'` both hold, and the `CHAR#` prefix says outright what the slot already decides.

A `CHAR` is one character of any script, stored as its code point in a 32-bit slot: `'é'` and `'中'` are fine, `'ab'` is refused (E0313), whichever form wrote it.
A `CHAR` *value* does not widen to `STRING`; `CHAR_TO_STRING` writes its UTF-8 bytes, `CHAR_TO_BYTE` keeps its low byte and `BYTE_TO_CHAR` reads one.

A string literal longer than the destination's declared capacity is refused at compile time (E0314), not truncated.

## Date and time encodings

Each date/time type is a fixed integer encoding, and the compiler refuses any literal outside its range (E0306, with the bounds shown as literals).

| Type | Encoding | Range |
| --- | --- | --- |
| `TIME` | i32 ms | about -24d20h to +24d20h |
| `LTIME` | i64 ns | about 292 years either way |
| `DATE` | i32 days since 1970-01-01 | no practical limit |
| `LDATE` | i64 days since 1970-01-01 | no practical limit |
| `TOD` | i32 ms since midnight | one day |
| `LTOD` | i64 ns since midnight | one day |
| `DT` | i64 seconds since 1970-01-01 | 1677-09-21 to 2262-04-11 (LDT's span) |
| `LDT` | i64 ns since 1970-01-01 | 1677-09-21 to 2262-04-11 |

`DT` shares `LDT`'s span (its bounds are that range in whole seconds), so the implicit widening never overflows and there is no 2038 problem; the pair differs by precision, seconds versus nanoseconds.

All of them are zone-naive: no timezone, no DST, no leap seconds.
A `DT` literal is mapped to its epoch value as UTC, so any future host clock source must deliver UTC or stored timestamps will be off by the local offset.

Conversions, `TO_STRING` formatting, timers and the full policy live in `programming-time`.

`REAL_TO_STRING` and `LREAL_TO_STRING` print the shortest text that reads back to the same value, spelled as a REAL literal: `'1.5'`, `'50.1'`, `'100.0'`, `'1.0E20'`, `'2.5E-9'`.
A REAL is formatted as the REAL it is, so `50.1` is `'50.1'` and not its f64 widening; an LREAL shows every digit it holds, so `0.1 + 0.2` is `'0.30000000000000004'`.
`'NaN'`, `'Inf'` and `'-Inf'` spell the values no literal can.

## Type declarations

Everything below lives inside a `TYPE … END_TYPE` block.

```iecst
TYPE
	Color: (Red, Green, Blue);            // enum
	Mode: (Run, Stop) := Mode#Stop;       // enum with a default, written qualified
	Coded: INT(Off := 0, On := 10);       // enum with a base type and explicit values
	Level: INT(0..100);                   // subrange
	Point: STRUCT
		x: INT;
		y: INT;
	END_STRUCT;
	Vec: ARRAY[0..9] OF INT;
	Grid: ARRAY[1..3, 1..4] OF REAL;      // multi-dimensional
	Name: STRING[16];
	IntRef: REF_TO INT;
	Alias: INT;
END_TYPE
```

Enum values are **qualified only**: write `Color#Green`, never a bare `Green`.
A bare variant is E0201 "no item found in scope".
This holds in initializers, expressions and `CASE` labels alike.

Initializers:

```iecst decl
p: Point := (x := 1, y := 2);             // struct: parentheses, named fields
v: Vec := [1, 2, 3, 4, 5, 5(0)];          // array: brackets; N(x) repeats x N times
m: ARRAY[0..1, 0..1] OF INT := [[1, 2], [3, 4]];
o: Outer := (i := (a := 1, b := [1, 2]), name := 'hi');   // nested
```

References use `REF()`, `^` and `NULL`:

```iecst fragment
r := REF(target);
r^ := 42;
IF r <> NULL THEN r^ := 0; END_IF;
```

Bit and slice access on bit-string types: `w.3` and `w.%X3` are the bit (a `BOOL`), `l.%B7` a byte, `l.%W3` a word, `l.%D1` a double word.
The slice takes the slice's type, not the base's.
