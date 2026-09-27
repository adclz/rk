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
