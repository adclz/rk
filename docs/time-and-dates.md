# Time and dates

Each date and time type is a fixed integer encoding:

| Type | Encoding | Range |
|---|---|---|
| `TIME` | i32 milliseconds | about -24d20h to +24d20h |
| `LTIME` | i64 nanoseconds | about 292 years either way |
| `DATE` | i32 days since 1970-01-01 | no practical limit |
| `LDATE` | i64 days since 1970-01-01 | no practical limit |
| `TOD` | i32 milliseconds since midnight | one day |
| `LTOD` | i64 nanoseconds since midnight | one day |
| `DT` | i64 seconds since 1970-01-01 | 1677-09-21 to 2262-04-11 |
| `LDT` | i64 nanoseconds since 1970-01-01 | 1677-09-21 to 2262-04-11 |

They have no timezone, no daylight saving and no leap seconds.
A `DT` literal is read as UTC.

```iecst decl
t : TIME  := T#1d2h3m4s5ms;             // d h m s ms us ns, decimals allowed
l : LTIME := LT#500us;
d : DATE  := D#2024-01-31;
o : TOD   := TOD#12:30:45.123;
n : DT    := DT#2024-01-31-12:30:45;
```

A duration can be negative: `T#-5s`.
A literal outside its type's range is `E0306`.

## Arithmetic

Only durations of the same type add and subtract: `TIME + TIME`, `LTIME - LTIME`.
Anything else is `E0305`, `DATE - DATE` and `TOD + TIME` included.

```iecst expect=E0305
FUNCTION Demo
VAR
    a : DATE;
    b : DATE;
    t : TIME;
END_VAR
    t := T#1s + T#500ms;   // Ok
    t := a - b;            // E0305
END_FUNCTION
```

To compute with the others, convert to the encoding and back:

```iecst fragment
now := DINT_TO_TOD(TOD_TO_DINT(now) + TIME_TO_DINT(delta));
```

The wrap past midnight is yours to handle.

## Conversions

Widening to the `L` type is implicit: `TIME` to `LTIME`, `DATE` to `LDATE`, `TOD` to `LTOD` and `DT` to `LDT`.
Everything else is a function of `Std.Convert`, like `DT_TO_DATE` or `LTIME_TO_TIME`.

`TIME_TO_STRING` and its siblings print the literal form: `TIME_TO_STRING(T#90s)` is `'T#1m30s'`.

The timers `TON`, `TOF` and `TP` are in `Std.Timers`.
