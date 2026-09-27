# Enums

An enum lists its values in a `TYPE` block.

```iecst
TYPE
    Color : (Red, Green, Blue);
    Mode : (Run, Stop) := Mode#Stop;        // a default value
END_TYPE
```

A value is always written with its type: `Color#Green`.
A bare `Green` is `E0201`, in expressions, initializers and `CASE` labels alike.

```iecst expect=E0201
TYPE Color : (Red, Green, Blue); END_TYPE

FUNCTION Demo
VAR
    c : Color;
END_VAR
    c := Color#Green;   // Ok
    c := Green;         // E0201
END_FUNCTION
```

## Values and storage

An enum is stored as a `DINT`, unless you give it a base type.
The base type must be an integer type (`E0601`).

```iecst
TYPE
    Small : SINT (Lo, Hi);                  // one byte
    Coded : INT (Off := 0, On := 10);       // explicit values
    Step : (A := 5, B, C);                  // B is 6 and C is 7
END_TYPE
```

A value without one continues from the value before it.
An explicit value must be a constant, and arithmetic over constants is fine (`E0604`).

## CASE

`CASE` takes the values as labels, each written with its type:

```iecst
TYPE Color : (Red, Green, Blue); END_TYPE

FUNCTION Code : INT
VAR_INPUT
    c : Color;
END_VAR
    CASE c OF
        Color#Red:   Code := 1;
        Color#Green: Code := 2;
    ELSE
        Code := 0;
    END_CASE;
END_FUNCTION
```
