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
| a `FUNCTION_BLOCK` `VAR_IN_OUT` of a scalar type | a constant, as a member |
| a `VAR_TEMP`, a `VAR_EXTERNAL`, or any other `VAR_IN_OUT` | none |

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

- **Inside the block**, the body and the methods read the edge. It is computed once, when the call starts.
- **Outside the block**, `counter.pulse` is the value the caller passed.
- **The first call** counts as an edge when the input is already `TRUE`, as with `R_TRIG`. For `F_EDGE`, it counts when the input is already `FALSE`, as with `F_TRIG`.
- **The previous value** is stored in the instance. A retained instance keeps it across a power cycle. A copy of the instance carries it too.

A cold start, or a warm start of an instance that is not retained, starts the previous value over.
An input held `TRUE` through the restart then reads as a rising edge on the first scan, as with `R_TRIG`. This is intended.

Inside its block, an edge input cannot be written, referenced or passed to a `VAR_IN_OUT` (`E0211`).
A `FUNCTION` or a `METHOD` keeps nothing between calls and cannot declare an edge input (`E0210`).

## VAR_IN_OUT in a function block

A `FUNCTION_BLOCK` keeps a `VAR_IN_OUT` of a scalar type as a member: a fixed-size elementary type, an enum or a subrange.
A call that binds it copies the argument in before the body and back after it, in the order the members are declared.
A call may leave it out, and the body then reads what the member holds: its initial value, or what the last call left.
The section takes `RETAIN` or `NON_RETAIN` like a `VAR`.
The member goes beyond the standard, which binds every `VAR_IN_OUT` at each call.

```iecst
FUNCTION_BLOCK Counter
VAR_IN_OUT RETAIN
    count : INT := 0;
END_VAR
    count := count + 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    c : Counter;
    n : INT;
END_VAR
    c();            // counts in the member
    c(count := n);  // n comes in, and takes the count back
END_PROGRAM
```

A `STRING`, a `STRUCT`, an `ARRAY`, an instance or a reference stays bound to the caller's variable at each call, as every `VAR_IN_OUT` of a `FUNCTION` or a `METHOD` is: a call supplies it (`E0802`), and it takes no initial value (`E0004`) and no qualifier (`E0906`).
The copy lands when the call ends, a `RETURN` included: a body that raises writes nothing back, a variable bound to two members keeps the one declared last, and `REF()` of a member points at the member.
A bit, `%QX0.3` beside a `%QW0` or `w.%X3`, can be bound to a member as to an output; a `VAR_IN_OUT` bound by reference takes no bit (`E1423`).
