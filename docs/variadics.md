# Variadics

A FUNCTION or a METHOD can take any number of arguments of one type, collected in a variadic parameter.

Declare it with `...` after its type, and read it with a fold or by position: `...values+` adds every argument, `...values[0]` reads the first.

```iecst
FUNCTION Sum : INT
VAR_INPUT
    values : INT...;
END_VAR
    Sum := ...values+;
END_FUNCTION

FUNCTION Demo : INT
    Demo := Sum(1, 2, 3) + Sum(10, 20) + Sum(7);   // 6 + 30 + 7
END_FUNCTION
```

The compiler makes one copy of `Sum` per argument count, so each call runs on plain parameters, with no array.
A METHOD gets its copies the same way, per argument count and per instance type it runs on.

## Folds

- **Arithmetic** `+ - * / % **` combines the arguments from left to right: `...values-` on `10, 3, 2` is `(10 - 3) - 2`.
- **Logic** `& | ^` is the AND, OR or XOR of every argument, on `BOOL` and bit strings.
- **Comparison** `= <> < > <= >=` tests each adjacent pair and gives a `BOOL`: `...values<` is `TRUE` when the arguments are in ascending order.
- **Extremes** `MIN MAX` gives the smallest or the largest argument: `...values MIN`, with a space, since the operator is a word. Each step keeps the value so far when it is `<=` the next argument, or `>=` for `MAX`, and takes that argument otherwise.

With a single argument, an arithmetic fold, `MIN` and `MAX` give that argument, and a comparison fold gives `TRUE`.

## By position

`...values[i]` reads one argument, counting from 0. The index is any integer: one below 0 reads the first argument, and one past the last reads the last.

```iecst
FUNCTION Pick : INT
VAR_INPUT
    i : LINT;
    values : INT...;
END_VAR
    Pick := ...values[i];
END_FUNCTION

FUNCTION Demo : INT
    Demo := Pick(1, 10, 20, 30) + Pick(9, 10, 20, 30);   // 20 + 30
END_FUNCTION
```

The inputs declared before the variadic parameter take their arguments first, here `i`.

## Rules

- The type must be elementary, or an alias of one, and neither a `STRING` nor a subrange (`E0811`).
- A FUNCTION or METHOD has one variadic parameter (`E0812`), and it is its last `VAR_INPUT` (`E0815`).
- A PROGRAM or a FUNCTION_BLOCK takes none (`E0029`).
- A call passes it at least one argument (`E0813`).
- `...` reads the POU's own variadic parameter and nothing else (`E0814`).
- `...` is the only way to read it: `First := values`, `values := 0` or `Other(values)` is `E0816`.
- An index has an integer type (`E0819`).
- An `{export}` FUNCTION cannot be variadic, since each argument count is a function of its own (`E1509`).

Each argument converts to the parameter's type as any input does, see [Strict casts](strict-casts.md).

Other parameters go in `VAR_OUTPUT` or `VAR_IN_OUT` and are passed by name, anywhere in the call:

```iecst
FUNCTION Sum : INT
VAR_INPUT
    values : INT...;
END_VAR
VAR_IN_OUT
    calls : INT;
END_VAR
    calls := calls + 1;
    Sum := ...values+;
END_FUNCTION

FUNCTION Demo : INT
VAR
    n : INT;
END_VAR
    Demo := Sum(1, 2, 3, calls := n);
END_FUNCTION
```
