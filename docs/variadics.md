# Variadics

A FUNCTION can take any number of arguments of one type, collected in a variadic parameter.

Declare it with `...` after its type, and read it with a fold: `...values+` adds every argument.

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

## Folds

- **Arithmetic** `+ - * / % **` combines the arguments from left to right: `...values-` on `10, 3, 2` is `(10 - 3) - 2`.
- **Logic** `& | ^` is the AND, OR or XOR of every argument, on `BOOL` and bit strings.
- **Comparison** `= <> < > <= >=` tests each adjacent pair and gives a `BOOL`: `...values<` is `TRUE` when the arguments are in ascending order.

With a single argument, an arithmetic fold gives that argument and a comparison fold gives `TRUE`.

## Rules

- The type must be elementary, or an alias of one (`E0811`).
- A FUNCTION has one variadic parameter (`E0812`), and it is its only `VAR_INPUT` (`E0815`).
- A call passes it at least one argument (`E0813`).
- `...` folds a variadic parameter only (`E0814`).
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
