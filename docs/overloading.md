# Overloading

The IEC standard defines generics for **ANY_INT**, **ANY_MAGNITUDE** etc ...
Those are normally reserved for the standard library,
but it turns out that using overloads can mimic this system by writing each possible variant.

This comes with the advantage that the compiler does not need extra plumbing for these generics,
because the behavior is implemented directly in **ST**.

>[!IMPORTANT]
>__Overloading can only be used on **FUNCTIONs**__

```st
FUNCTION MyFn
    VAR_INPUT
        Input: INT;
    END_VAR
END_FUNCTION

FUNCTION MyFn  // <-- Legal
    VAR_INPUT
        Input: REAL;
    END_VAR
END_FUNCTION
```

And then on usage:

```st
MyFn(0) <-- Will pick the first overload
MyFn(1.0) <-- Will pick the second overload
```

> [!CAUTION]
> **Return type** and **output types** affect the signature of overloads.

If a call site can fit multiple overloads, this will trigger an error:

```st
FUNCTION pick : INT
VAR_INPUT x : DINT; END_VAR
    pick := 1;
END_FUNCTION

FUNCTION pick : INT
VAR_INPUT x : LINT; END_VAR
    pick := 2;
END_FUNCTION

FUNCTION caller : INT
VAR y : SINT; END_VAR
    // SINT widens to both DINT and LINT, neither overload is exact.
    caller := pick(y);
END_FUNCTION
```

```sh
[E0809] Error: ambiguous overloaded call
    ╭─[ file:///example0.st:14:15 ]
    │
  1 │ FUNCTION pick : INT
    │          ──┬─  
    │            ╰─── a candidate is declared here
    │ 
  6 │ FUNCTION pick : INT
    │          ──┬─  
    │            ╰─── a candidate is declared here
    │ 
 14 │     caller := pick(y);
    │               ──┬─  
    │                 ╰─── call to 'pick' is ambiguous: 2 overloads accept these arguments
    │ 
    │ Help: pick one with a typed literal or a conversion, such as `DINT#5` or `INT_TO_DINT(x)`
    │ 
    │ Note: an argument widens to each overload
────╯
```

## Outputs

Overloads can also differ only in the types of their `VAR_OUTPUT`s.
When the arguments fit several overloads alike, the variables a call writes the outputs into pick one:

```iecst
USING Std.Convert;

FUNCTION Split
VAR_INPUT IN : DINT; END_VAR
VAR_OUTPUT HIGH, LOW : INT; END_VAR
    HIGH := DINT_TO_INT(IN / 1000);
    LOW := DINT_TO_INT(IN MOD 1000);
END_FUNCTION

FUNCTION Split
VAR_INPUT IN : DINT; END_VAR
VAR_OUTPUT HIGH, LOW : DINT; END_VAR
    HIGH := IN / 1000;
    LOW := IN MOD 1000;
END_FUNCTION

FUNCTION Example : DINT
VAR small : INT; large : DINT; END_VAR
    Split(IN := 70000, LOW => small);  // the INT overload
    Split(IN := 70000, LOW => large);  // the DINT overload
    Example := small + large;
END_FUNCTION
```

An output of the variable's own type is an exact match.
An output that widens into the variable is a widening, as for an argument.
An overload whose output does not convert into the variable is not a candidate.
A call that writes none of the outputs the overloads differ in is ambiguous (`E0809`).
