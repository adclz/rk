## Elementary types

`BOOL`, `BYTE`, `WORD`, `DWORD`, `LWORD`,
`SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`,
`REAL`, `LREAL`,
`TIME`, `LTIME`, `DATE`, `LDATE`, `TIME_OF_DAY` (`TOD`), `LTIME_OF_DAY` (`LTOD`), `DATE_AND_TIME` (`DT`), `LDATE_AND_TIME` (`LDT`),
`STRING`, `STRING[n]`, `CHAR`.

There is no `WSTRING`. A bare `STRING` has a capacity of 80 bytes.

Widening within the same family is implicit (`INT` → `DINT`, `INT` → `REAL`). Everything else — narrowing, signed/unsigned, integer/bit-string, real → integer — needs an explicit `Std.Convert` call, and E0301 names the one to use: `INT_TO_SINT(i)`, `REAL_TO_INT(r)`, `INT_TO_WORD(i)`.

Real → integer rounds to nearest, ties to even (`REAL_TO_INT(2.5)` is `2`, `REAL_TO_INT(3.5)` is `4`); `TRUNC` drops the fraction instead. A value outside the target's range saturates to the nearest bound and NaN converts to `0`; `Std.Math` has `IS_NAN` and `NOT_OK` (NaN or infinite) to test a value before converting it.

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
c: CHAR := CHAR#'A';        // a bare 'A' is a STRING, not a CHAR
```

Inside a single-quoted string, `$` escapes: `$$`, `$'`, `$L`, `$N`, `$P`, `$R`, `$T`, and `$41` for one hex byte. Double-quoted strings are also accepted and produce a `STRING`.

A literal's characters are their UTF-8 bytes, which is how the runtime, the debugger and the network read a `STRING`: `'café'` is 5 bytes, `LEN` counts bytes, and a `STRING[4]` refuses it (E0309).

A `CHAR` is one character of any script, stored as its code point in a 32-bit slot: `CHAR#'é'` and `CHAR#'中'` are fine, `CHAR#'ab'` is refused. It does not widen to `STRING` by itself; `CHAR_TO_STRING` writes its UTF-8 bytes, `CHAR_TO_BYTE` keeps its low byte and `BYTE_TO_CHAR` reads one.

A string literal longer than the destination's declared capacity is refused at compile time (E0309), not truncated.

## Date and time encodings

Each date/time type is a fixed integer encoding, and the compiler refuses any literal outside its range (E0309, with the bounds shown as literals).

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

All of them are zone-naive: no timezone, no DST, no leap seconds. A `DT` literal is mapped to its epoch value as UTC, so any future host clock source must deliver UTC or stored timestamps will be off by the local offset.

Conversions, `TO_STRING` formatting, timers and the full policy live in `programming-time`.

## Type declarations

Everything below lives inside a `TYPE … END_TYPE` block.

```iecst
TYPE
	Color: (Red, Green, Blue);            // enum
	Coded: INT(Off := 0, On := 10);       // enum with a base type and explicit values
	Level: INT(0..100);                   // subrange
	Point: STRUCT
		x: INT;
		y: INT;
	END_STRUCT;
	Overlaid: STRUCT OVERLAP              // parsed, but fields do NOT share storage yet
		raw: DWORD;
		f: REAL;
	END_STRUCT;
	Vec: ARRAY[0..9] OF INT;
	Grid: ARRAY[1..3, 1..4] OF REAL;      // multi-dimensional
	Name: STRING[16];
	IntRef: REF_TO INT;
	Alias: INT;
END_TYPE
```

Enum values are **qualified only**: write `Color#Green`, never a bare `Green`. A bare variant is E0204 "no item found in scope". This holds in initializers, expressions and `CASE` labels alike.

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

Bit and slice access on bit-string types: `w.3` and `w.%X3` are the bit (a `BOOL`), `l.%B7` a byte, `l.%W3` a word, `l.%D1` a double word. The slice takes the slice's type, not the base's.
