# Syntax

- Unlike traditional ST compilers, semicolons `;` are not mandatory.

The following program can compile without any problem:
```st
FUNCTION MyFn
    VAR
        test: INT //;
        test2: INT //;
    END_VAR //;

    IF test > test2 THEN
        //;
    END_IF  //;
END_FUNCTION

```

The formatter writes the missing ones in.

A `;` on its own is the empty statement and is accepted wherever a statement is, `IF ready THEN ; END_IF` included.

- Keywords and identifiers are case-insensitive: `myFn` and `MyFn` are one name.

- Enum values are always qualified: `Color#Green`. A bare `Green` is `E0201`.

## Initial values

Where a variable is declared decides what its initial value can be.

| Declared in | Initial value |
| --- | --- |
| a `TYPE`, a STRUCT field, an FB or CLASS member, a `PROGRAM`, a `VAR_GLOBAL` | a constant |
| a `FUNCTION` or `METHOD` `VAR` or `VAR_OUTPUT` | any expression, computed at every call, naming only variables declared before it |
| a `FUNCTION` or `METHOD` `VAR_INPUT` | a constant, which the caller passes when the argument is omitted |
| a `VAR_TEMP`, `VAR_IN_OUT` or `VAR_EXTERNAL` | none |

A constant is a literal, an enum value, `NULL`, a `CONSTANT` declared beside it or imported with `VAR_EXTERNAL CONSTANT`, arithmetic over those, or `REF()` of a global.
A member can also point at a member beside it, in its own instance: `p : REF_TO INT := REF(x)`.
Anything else is `E0401`.

A `FUNCTION` or `METHOD` gives its variables their values at every call, in the order they are declared.
An initial value can name another of its variables only if that one is declared first (`E0406`):

```iecst expect=E0406
FUNCTION Perimeter : INT
VAR_INPUT w : INT; h : INT; END_VAR
VAR
    p : INT := 2 * s;  // <-- E0406: s is declared after p
    s : INT := w + h;
END_VAR
    Perimeter := p;
END_FUNCTION
```

Inputs, `VAR_EXTERNAL`s, `CONSTANT`s and `REF()` can be named in any order.

## CASE

`CASE` branches on an integer, a bit string, a `CHAR`, an enum or a `STRING`, and each label is a constant.

A literal selector takes its type from the labels, as the two sides of `=` do:

```iecst
FUNCTION Pick : INT
    CASE 5 OF
        DINT#5: Pick := 1; // <-- compares as DINT
    ELSE
        Pick := 0;
    END_CASE;
END_FUNCTION
```

With only literals for labels, it is an `INT` or a `STRING`, like the operands of `200 * 200` in [Math operations](math-operations.md).

## Edge inputs

An input of a `FUNCTION_BLOCK` or a `PROGRAM` declared `R_EDGE` reads as its rising edge: `TRUE` for the one call where it went from `FALSE` to `TRUE`.
`F_EDGE` reads the falling edge.

```iecst
FUNCTION_BLOCK Counter
VAR_INPUT
    pulse : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    count : UINT;
END_VAR
    IF pulse THEN // <-- once per press, however many calls it is held
        count := count + 1;
    END_IF;
END_FUNCTION_BLOCK
```

The block's own code reads the edge, its methods included.
A caller still passes the input's value, and reads it back as it gave it.
The first call counts as an edge when the input is already `TRUE`, as with `R_TRIG`, and already `FALSE` for `F_EDGE`, as with `F_TRIG`.

A `FUNCTION` or a `METHOD` keeps nothing from one call to the next, so it has no edge input (`E0210`).
