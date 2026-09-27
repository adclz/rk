# Subranges

A subrange narrows an integer type to the values between two bounds.

```iecst
TYPE
    Percent : INT(0..100);
END_TYPE
```

The base must be an integer type (`E0701`), and both bounds must be constants (`E0703`).
A variable can also be declared with one directly: `level : UINT(0..10);`.

A constant outside the bounds is refused at compile time:

```iecst expect=E0702
TYPE Percent : INT(0..100); END_TYPE

FUNCTION Demo
VAR
    p : Percent;
END_VAR
    p := 50;    // Ok
    p := 150;   // E0702
END_FUNCTION
```

Any other value is checked when it is stored, and raises `value out of subrange bounds`.
See [Bundled Traps](bundled-traps.md).

A subrange reads as its base type, so `n := p;` needs no conversion when `n` is an `INT`.

## By reference

A `VAR_IN_OUT` or an output binds the caller's own variable, so both sides must have the same subrange (`E0704`).
Otherwise the callee could write 500 into a `Percent` through a plain `INT`.

```iecst expect=E0704
TYPE Percent : INT(0..100); END_TYPE

FUNCTION Double
VAR_IN_OUT
    n : INT;
END_VAR
    n := n * 2;
END_FUNCTION

FUNCTION Demo
VAR
    p : Percent := 60;
END_VAR
    Double(n := p);   // E0704
END_FUNCTION
```
