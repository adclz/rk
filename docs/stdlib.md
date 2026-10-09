# StdLib

The standard library is written in plain Structured Text, and you will find it in `stdlib/`.
Each file is a namespace of its own:

```schema
stdlib/                             one file per namespace
├─ Std.Math                         SQRT, SIN, EXPT, …
├─ Std.Strings                      LEN, CONCAT, CHAR_AT, …
├─ Std.Convert                      DINT_TO_REAL and every other cast
├─ Std.Selection                    MIN, MAX, LIMIT, SEL, MUX
├─ Std.Timers                       TON, TOF, TP
├─ Std.Counters                     CTU, CTD, CTUD
├─ Std.Edge                         R_TRIG, F_TRIG
├─ Std.Bistable                     SR, RS
├─ Std.Bits                         SHL, SHR, ROL, ROR
├─ Std.Bytes                        TO_BIG_ENDIAN, WORD_BCD_TO_UINT, …
├─ Std.Memory                       MOVE
├─ Std.Arrays                       LOWER_BOUND, UPPER_BOUND
└─ Std.Unit                         ASSERT, ASSERT_EQ, ASSERT_NEQ
```

The compiler knows nothing about it beyond where the files are.

So that means you can replace any part of the stdlib, extend it, or read it to see how a `TON` is written.
