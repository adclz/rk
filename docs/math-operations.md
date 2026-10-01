# Math operations

Every math function is compiled into your module: the host has nothing to provide.
More importantly, a result never depends on the platform: the same program computes the same values on every runtime, on any machine.

The functions live in `Std.Math`:

```iecst
USING Std.Math;

FUNCTION Hypotenuse : REAL
VAR_INPUT
    a : REAL;
    b : REAL;
END_VAR
    Hypotenuse := SQRT(IN := a * a + b * b);
END_FUNCTION
```

- `+`, `-`, `*`, `/`, `MOD`, `SQRT` and `ABS` are one WebAssembly instruction each.
- `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `ATAN2`, `EXP`, `LN`, `LOG` and `**` come from libm, and are added to the module only when you call them.

`BYTE`, `WORD`, `DWORD` and `LWORD` take `+`, `-`, `*` and `/` as unsigned integers of their width.
IEC 61131-3 gives bit strings no arithmetic: this is an rk extension.

## When a result does not fit

An integer that goes past its limit wraps around, without an error:

```iecst fragment
i := 32767;  // an INT
i := i + 1;  // <-- i is now -32768
```

Arithmetic on constants wraps the same way wherever it is written: in an initializer, a `CASE` label or an array bound.
`x : DINT := 200 * 200` multiplies two `INT`s and gives -25536; write `DINT#200 * 200` to compute in `DINT`.

A float never fails: an impossible result is a value you can test.

- The square root of a negative number is NaN, and `IS_NAN` tells you.
- The logarithm of `0.0` is minus infinity.

Converting a float to an integer never fails either:

- NaN gives `0`.
- A value too large gives the largest one the integer holds: `REAL_TO_INT` of `1.0E6` is `32767`.

The one thing that stops the module is an integer divided by zero, see [Bundled traps](bundled-traps.md).

## Powers

`**` takes a `REAL` or an `LREAL` base, and returns the same type:

```iecst fragment
r := 2.0 ** 3;  // <-- 8.0
x := 2 ** 3;    // <-- E0305: the base is an integer
```

`EXPT` is the same function, under its standard name.

## Mixing types

There is no `ANY_REAL` or `ANY_INT` in the type system, see [Overloading](overloading.md).
`Std.Math` writes each function out once per type, in plain ST: you can read it, and replace it.

Implicit casts follow the standard's table, stricter than most toolchains:

```iecst expect=E0301
FUNCTION Casts
VAR
    i : INT;
    d : DINT;
    r : REAL;
END_VAR
    r := i;  // <-- INT to REAL: fine
    r := d;  // <-- E0301: the table has no DINT to REAL
    i := r;  // <-- E0301: never implicit
END_FUNCTION
```

The error names the cast to write, like `DINT_TO_REAL(d)`.
[Strict casts](strict-casts.md) has the whole table.

> [!TIP]
> A literal computes at its own type first: `x : LREAL := 0.1 + 0.0` adds two `REAL`s, then widens the result.
> Write `LREAL#0.1 + 0.0` to compute in `LREAL`.
