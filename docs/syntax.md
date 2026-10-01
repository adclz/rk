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
