# Exceptions

An exception is an error that happens while the program runs.

It stops the code where it happens, then goes back up through the callers until one of them catches it.
If none of them does, the scan stops.

An exception carries a message, a `STRING` that says what went wrong.

- **`__RAISE`** raises an exception.
- **`__TRY` and `__CATCH`** catch it.
- **`__FINALLY`** runs some code whether an exception was raised or not.

```iecst
FUNCTION Ratio : DINT
VAR_INPUT part : DINT; whole : DINT; END_VAR
    IF whole = 0 THEN
        __RAISE('nothing to divide by');
    END_IF;
    Ratio := part * 100 / whole;
END_FUNCTION

FUNCTION Share : DINT
VAR_INPUT part : DINT; whole : DINT; END_VAR
VAR why : STRING; END_VAR
    __TRY
        Share := Ratio(part, whole);
    __CATCH(why)
        Share := -1; // <-- why is 'nothing to divide by'
    __END_TRY;
END_FUNCTION
```

> [!NOTE]
> Exceptions are not part of IEC 61131-3, they are an rk extension.

## __RAISE

`__RAISE` takes a `STRING` message.

- **The statements after it** are not executed.
- **The callers** are stopped too, up to the first `__TRY`.
- **Variables written before it** keep their new value.
- **Without a `__TRY`** the scan stops. In a `{test}`, the test fails with the message.

The checks the compiler inserts raise the same way, see [Bundled Traps](bundled-traps.md).
A failed assertion does too, see [Tests](tests.md).

## __TRY and __CATCH

The `__CATCH` block only runs if something was raised in the `__TRY` block.
The program then continues after `__END_TRY`.

The variable in `__CATCH(why)` receives the message.
It must be a `STRING`, and a message longer than the variable is cut.

You can write `__CATCH` without a variable if you do not need the message.

A `__TRY` can only have one `__CATCH` (`E1212`).
It catches anything raised in the `__TRY` block, including in the functions it calls.

To pass an exception to the caller, raise it again:

```iecst
FUNCTION Load : DINT
VAR_INPUT slot : DINT; END_VAR
    IF slot > 3 THEN
        __RAISE('no such slot');
    END_IF;
    Load := slot * 10;
END_FUNCTION

FUNCTION LoadOrZero : DINT
VAR_INPUT slot : DINT; END_VAR
VAR why : STRING; END_VAR
    __TRY
        LoadOrZero := Load(slot);
    __CATCH(why)
        IF why <> 'no such slot' THEN
            __RAISE(why); // <-- goes to the caller
        END_IF;
        LoadOrZero := 0;
    __END_TRY;
END_FUNCTION
```

## __FINALLY

The `__FINALLY` block always runs, whether something was raised or not.

```iecst
FUNCTION_BLOCK Press
VAR_INPUT force : DINT; END_VAR
VAR_OUTPUT busy : BOOL; cycles : DINT; END_VAR
    busy := TRUE;
    __TRY
        IF force > 1000 THEN
            __RAISE('too much force');
        END_IF;
        cycles := cycles + 1;
    __FINALLY
        busy := FALSE; // <-- runs in both cases
    __END_TRY;
END_FUNCTION_BLOCK
```

It runs:

- **After the `__TRY` block** if nothing was raised.
- **After the `__CATCH` block** if something was raised.
- **Before the exception goes to the caller** if there is no `__CATCH`.
- **Before a `RETURN`, `EXIT` or `CONTINUE`** written in the `__TRY` or `__CATCH` block.

You cannot leave a `__FINALLY` with `RETURN`, `EXIT` or `CONTINUE` (`E1210`).

If you `__RAISE` inside a `__FINALLY`, the new exception replaces the previous one.

A `__TRY` needs a `__CATCH`, a `__FINALLY` or both (`E1209`).
With both, `__CATCH` is written first (`E1211`).

## In a PROGRAM

A `PROGRAM` can catch an exception to finish its scan:

```iecst
FUNCTION_BLOCK Counter
VAR_OUTPUT n : DINT; END_VAR
    n := n + 1;
    IF n = 2 THEN
        __RAISE('two');
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    failures : DINT;
    last : STRING;
END_VAR
    __TRY
        c();
    __CATCH(last)
        failures := failures + 1; // <-- on the second scan
    __END_TRY;
END_PROGRAM
```

`c.n` stays at 2 after the exception.
A `__CATCH` does not undo what was written before the `__RAISE`.

## In a test

A test that catches its exception passes.
You can use this to check that a function raises:

```iecst
USING Std.Unit;

FUNCTION Half : DINT
VAR_INPUT n : DINT; END_VAR
    IF n MOD 2 <> 0 THEN
        __RAISE('odd');
    END_IF;
    Half := n / 2;
END_FUNCTION

{test}
FUNCTION test_half_refuses_an_odd_number
VAR why : STRING; END_VAR
    __TRY
        Half(3);
    __CATCH(why)
    __END_TRY;
    ASSERT_EQ(why, 'odd');
END_FUNCTION
```

## Division by zero

Dividing an integer by zero is not an exception, it is a trap of the VM.

The call is stopped and neither `__CATCH` nor `__FINALLY` runs.
Check the divisor first if it can be zero.

## WebAssembly

`__RAISE` is a WASM [throw](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/throw) and `__TRY` is a [try_table](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/try_table).

See [Bundled Traps](bundled-traps.md) for the exception tag and how a host reads the message.
