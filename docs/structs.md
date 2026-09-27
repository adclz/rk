# Structs

A `STRUCT` is declared in a `TYPE` block, and each field can have a default value.

```iecst
TYPE
    Point : STRUCT
        x : INT := 3;
        y : INT;
    END_STRUCT;
END_TYPE
```

An initializer names the fields it sets, in parentheses.
A field it leaves out keeps its default.

```iecst continues
FUNCTION Demo
VAR
    a : Point := (x := 1, y := 2);
    b : Point := (y := 9);          // b.x is 3
END_VAR
END_FUNCTION
```

Structs nest, and so do their initializers:

```iecst
TYPE
    Axis : STRUCT
        position : REAL;
        limits : ARRAY[0..1] OF REAL := [-100.0, 100.0];
    END_STRUCT;
    Machine : STRUCT
        x : Axis;
        name : STRING[16] := 'press';
    END_STRUCT;
END_TYPE

PROGRAM Press
VAR
    m : Machine := (x := (position := 5.0, limits := [0.0, 50.0]));
END_VAR
    m.x.position := m.x.position + 1.0;
END_PROGRAM
```

A struct is a value:

- **Assigning** a struct copies every field.
- **Returning** one from a `FUNCTION` gives the caller its own copy.

```iecst
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

FUNCTION MakePoint : Point
VAR_INPUT x : INT; y : INT; END_VAR
    MakePoint.x := x;
    MakePoint.y := y;
END_FUNCTION

FUNCTION Demo : INT
VAR p : Point; q : Point; END_VAR
    p := MakePoint(x := 1, y := 2);
    q := p;         // q is a copy
    q.x := 10;      // p.x is still 1
    Demo := p.x;
END_FUNCTION
```
