# Arrays

An array has constant bounds, and the lower one can be any integer, negative included.

```iecst
FUNCTION Demo
VAR CONSTANT
    N : INT := 8;
END_VAR
VAR
    readings : ARRAY[0..9] OF REAL;
    offsets  : ARRAY[-5..5] OF INT;
    buffer   : ARRAY[0..N - 1] OF BYTE;     // a named constant, and arithmetic on it
    grid     : ARRAY[1..3, 1..4] OF DINT;   // two dimensions
END_VAR
END_FUNCTION
```

Elements can be of any type: structs, strings and function block instances included.

## Initializers

The elements go in brackets.
`N(x)` repeats `x` N times, and `N(a, b)` repeats the whole group.

```iecst
FUNCTION Demo
VAR
    a : ARRAY[0..9] OF INT := [1, 2, 3, 7(0)];          // 1, 2, 3, then seven 0
    b : ARRAY[0..5] OF INT := [2(1, 2, 3)];             // 1, 2, 3, 1, 2, 3
    m : ARRAY[0..1, 0..2] OF INT := [1, 2, 3, 4, 5, 6];
    n : ARRAY[0..1, 0..1] OF INT := [[1, 2], [3, 4]];
END_VAR
END_FUNCTION
```

A multi-dimensional array is filled last dimension first: in `m` above, `m[0, 2]` is 3 and `m[1, 0]` is 4.

```diagram
m : ARRAY[0..1, 0..2] OF INT := [1, 2, 3, 4, 5, 6]

                  second index
                 0     1     2
              ┌─────┬─────┬─────┐
first      0  │  1  │  2  │  3  │   filled first
index         ├─────┼─────┼─────┤
           1  │  4  │  5  │  6  │   then this row
              └─────┴─────┴─────┘
```

A bracket inside the brackets is one row, and a short row leaves the rest of it at the default: `[[1, 2], [3, 4, 5]]` fills the rows of `m` with 1, 2, 0 and 3, 4, 5.
When the element is itself an array, declared through a `TYPE`, a bracket is one element.

More elements than the array holds is `E0507`.

## Indexing

A subscript is an integer.
A `BOOL`, a `REAL` or a `STRING` cannot index an array (`E0504`).

`grid[2, 3]` and `grid[2][3]` are the same element.
`grid[2]` alone names a part of `grid`, and is refused (`E0510`).

An array of an array type has rows: each element is a whole array, and a second subscript indexes it.

```iecst
TYPE Row : ARRAY[1..4] OF DINT; END_TYPE

FUNCTION Demo : DINT
VAR
    rows : ARRAY[1..3] OF Row := [[1, 2, 3, 4], [5, 6]];   // one bracket per Row
    second : Row;
END_VAR
    second := rows[2];                  // 5, 6, 0, 0
    Demo := rows[2][2] + second[1];     // 6 + 5
END_FUNCTION
```

A constant subscript outside the bounds is refused at compile time:

```iecst expect=E0506
FUNCTION Demo : REAL
VAR
    readings : ARRAY[0..9] OF REAL;
END_VAR
    Demo := readings[10];   // E0506
END_FUNCTION
```

Any other subscript is checked when it runs, on each dimension, and raises `array index out of bounds`.
See [Bundled Traps](bundled-traps.md).

## Copies

Assigning an array copies every element.
An array assigns to another with the same bounds and element type, whether it is declared through a `TYPE` or written in place.
