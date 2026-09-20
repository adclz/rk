---
name: programming-time
description: Date and time in Structured Text — TIME/DATE/DT/TOD literals, their integer encodings, conversions, TO_STRING, and the TON/TOF/TP timers. Use when computing with time, formatting a timestamp, or timing something in a scan.
---

## Summary

Everything time-related in one place: the eight date/time types, their integer encodings and ranges, the conversion and formatting functions in `Std.Convert`, and the timers in `Std.Timers`.
The syntax basics are also in `programming-st`; this skill is the depth.

## Literals

```iecst decl
t: TIME := T#1d2h3m4s5ms;   // T# or TIME#; units d h m s ms us ns, decimals allowed (T#1.5s)
l: LTIME := LT#500us;       // LT# or LTIME#
d: DATE := D#2024-01-31;    // D# or DATE#; LD# or LDATE# for LDATE
o: TOD := TOD#12:30:45.123; // TOD# or TIME_OF_DAY#; LTOD# for LTOD
n: DT := DT#2024-01-31-12:30:45;  // DT# or DATE_AND_TIME#; LDT# for LDT
```

Durations may be negative (`T#-5s`, the sign goes after the `#`).
A bad or missing unit is E0309 and a malformed date E0310; each names the problem and ends with the shape a correct literal has.
An out-of-range literal is E0306, and shows the type's exact bounds as literals.

## Encodings and ranges

Each type is a fixed signed integer encoding.
The compiler refuses any literal outside the range.

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

`DT` and `LDT` share one span on purpose: DT's bounds are LDT's range in whole seconds, so the implicit `DT` → `LDT` widening can never overflow.
The pair differs by precision, not width: seconds versus nanoseconds.
There is no 2038 problem.

All of them are zone-naive: no timezone, no DST, no leap seconds.
A literal maps to its epoch value as UTC, so any future host clock source must deliver UTC or stored timestamps are off by the local offset.

Comparisons are signed, matching the encodings: `T#-5s < T#0s`, and a pre-epoch `D#1969-12-31` orders before `D#1970-01-01`.

Arithmetic exists for same-type durations only: `TIME + TIME`, `TIME - TIME` (and the `LTIME` pair).
Everything mixed is refused (E0305): no `DATE - DATE`, no `TOD + TIME`, no `TIME * INT`.
The workaround is the numeric conversions below: convert, compute in the encoding, convert back.
Duration arithmetic wraps at the lane like every other integer: the maximum `TIME` plus `T#1ms` is the minimum `TIME`.

## Conversions (`Std.Convert`)

Widening to the L-variant is implicit (`TIME` → `LTIME`, `TOD` → `LTOD`, `DATE` → `LDATE`, `DT` → `LDT`); the named functions `TIME_TO_LTIME`, `TOD_TO_LTOD`, `DATE_TO_LDATE`, `DT_TO_LDT` exist for when a call reads better.

Narrowing and decomposition are explicit: `LTIME_TO_TIME`, `LTOD_TO_TOD`, `LDATE_TO_DATE`, `LDT_TO_DT`, and from a timestamp `DT_TO_DATE`, `DT_TO_TOD`, `DT_TO_LDATE`, `DT_TO_LTOD`, `LDT_TO_DATE`, `LDT_TO_DT`, `LDT_TO_LDATE`, `LDT_TO_TOD`, `LDT_TO_LTOD`.

The decompositions floor, so they are exact everywhere including pre-epoch timestamps: `DT_TO_DATE(DT#1969-12-31-23:59:59)` is `D#1969-12-31` and the TOD half is always in-domain.
The one deliberate truncation is `LTIME_TO_TIME`, a duration narrowing (magnitude semantics).

Every type converts to its encoding number and back, zero cost:

```iecst fragment
ms  := TIME_TO_DINT(IN := T#1s500ms);   // 1500; DINT_TO_TIME reverses
d   := DATE_TO_DINT(IN := D#1970-01-02); // 1 day; DINT_TO_DATE reverses
s   := DT_TO_LINT(IN := dt_value);       // seconds since epoch (i64); LINT_TO_DT reverses
ns  := LDT_TO_LINT(IN := ldt_value);     // ns since epoch; LINT_TO_LDT reverses
// likewise LTIME_TO_LINT / LINT_TO_LTIME, TOD_TO_DINT / DINT_TO_TOD,
// LDATE_TO_LINT / LINT_TO_LDATE, LTOD_TO_LINT / LINT_TO_LTOD
```

This is the sanctioned way to do date/time arithmetic today: `DINT_TO_TOD(TOD_TO_DINT(t) + TIME_TO_DINT(delta))` adds a duration to a time of day (mind midnight wrap yourself).

## TO_STRING (`Std.Convert`)

Every date/time type formats to its IEC literal form, so the output parses back as a literal of the same type and value.
The spelling is canonical, not necessarily what was written: `TIME_TO_STRING(T#90s)` is `'T#1m30s'`.

```iecst fragment
TIME_TO_STRING(IN := T#1s500ms)                  // 'T#1s500ms'; zero is 'T#0s'
LTIME_TO_STRING(IN := LT#500us)                  // 'LT#500us'
DATE_TO_STRING(IN := D#2024-02-29)               // 'D#2024-02-29'
LDATE_TO_STRING(IN := LD#2024-02-29)             // 'LD#2024-02-29'
TOD_TO_STRING(IN := TOD#12:30:45.123)            // 'TOD#12:30:45.123'; whole seconds omit the fraction
LTOD_TO_STRING(IN := LTOD#12:30:45.123456789)    // nine-digit fraction when nonzero
DT_TO_STRING(IN := DT#2026-08-28-12:30:45)       // 'DT#2026-08-28-12:30:45'
LDT_TO_STRING(IN := LDT#2026-08-28-12:30:45.5)   // 'LDT#2026-08-28-12:30:45.500000000'
```

A negative TOD can no longer arise from a decomposition (they floor); should one ever reach `TOD_TO_STRING` through host-written memory, it prints with a leading `-`, which no TOD literal accepts, so a bad value stays visible.

## Timers (`Std.Timers`)

`TP_TIME`, `TON_TIME`, `TOF_TIME` and their `_LTIME` variants — pulse, on-delay, off-delay.
All share the interface `VAR_INPUT IN: BOOL; PT: TIME;` / `VAR_OUTPUT Q: BOOL; ET: TIME;` (or LTIME).
They read the host's monotonic clock through a WASI import, so they measure real elapsed time, not scan counts.

```iecst fragment
VAR debounce: Std.Timers.TON_TIME; END_VAR
debounce(IN := raw_input, PT := T#50ms);
IF debounce.Q THEN (* input stable for 50ms *) END_IF
```

`PLC_TIME: TIME` and `PLC_LTIME: LTIME` return the monotonic clock directly - time since an arbitrary start, good for measuring intervals, not wall-clock time.

## Gotchas

- No mixed arithmetic (E0305) - go through the numeric conversions.
- `T#2y` is not a duration - years and months are not IEC duration units.
- Timers need scans to update: `Q` changes on the call, not asynchronously.
