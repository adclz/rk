# Tests

Mark any `FUNCTION` with `{test}` and it becomes a test.
It checks its results with the assertions of `Std.Unit`:

```st
USING Std.Unit;

{test}
FUNCTION test_add
    ASSERT_EQ(Add(a := 2, b := 3), INT#5, 'Add(2, 3)');
END_FUNCTION
```

`rk test` compiles the workspace and runs all of them, showing a summary at the end.

```sh
    // ...
        PASS [   83µs] Std.Timers.Test.test_tp_ignores_input_during_pulse
        PASS [  1.1ms] Std.Timers.Test.test_tp_ltime_pulse
        PASS [  1.1ms] Std.Timers.Test.test_tp_post_pulse_retrigger
        PASS [  1.1ms] Std.Timers.Test.test_tp_pulse_expires
        PASS [  101µs] Std.Timers.Test.test_tp_pulse_starts
        PASS [   59µs] Std.Unit.Test.test_assert_eq_accepts_every_date_and_time_type
        PASS [   82µs] Std.Unit.Test.test_assert_neq_sees_a_different_date_or_time
────────────────────────────────────────────────────────────
    Summary [158.5ms] 341 tests run: 341 passed, 0 failed
```

## When a test fails

A test that fails is reported with the calls that were in progress, the innermost first.

```sh
     Failures:
        FAIL test_subrange (main.st:44): value out of subrange bounds
             at Clamp (main.st:6:5)
             at Twice (main.st:12:5)
             at test_subrange (main.st:45:5)
```

The line after the test's name is where it is declared, and each `at` line a statement that was running.
A file of the standard library is shown as `<lib>/Unit.st`.

It works the same for a failed assertion, a `__RAISE`, a check the compiler inserted, a trap of the VM and a test the watchdog stopped.
`rk test -O` reports it too: an optimized build has lost its line tables, so the failed test is run once more on the unoptimized one.

With `--output-format concise` the calls follow the reason on the same line, and with `json-lines` they are the record's `backtrace`.

The `Std.Unit` namespace contains three assertion FUNCTIONs, each one of them carries an optional **STRING** payload that the CLI or runtime should display in case of failure.

- `ASSERT` which asserts the condition being passed is TRUE:
```st
ASSERT(TRUE, "payload")
```

- `ASSERT_EQ` which asserts the left parameter is equal to the right parameter:
```st
ASSERT_EQ(1, 1, "1 + 1 should be 2")
```

- `ASSERT_NEQ`  which is the opposite of `ASSERT_EQ`.

All three use [overloading](overloading.md) so they can be used with all elementary types.

- Internally all assertions use the built-in `__RAISE` that triggers a WASM exception with a STRING payload
  
__Every test gets fresh instances, so tests never leak state into each other.__

>[!TIP]
> A test can be selected by its qualified path in the workspace, or by any part of it.
> ```st
>   NAMESPACE Ns1
>       NAMESPACE Tests
>       USING Std.Unit;
>       
>           {test}
>           FUNCTION MyTest 
>               ASSERT_EQ(1, 1, " 1 + 1 = 2")
>           END_FUNCTION
>
>       END_NAMESPACE
>   END_NAMESPACE
> ```
> can be accessed with `rk test`:
> 
>  `rk test Ns1.Tests.MyTest`
>
> The name is matched as a substring, so `rk test Tests` runs every test of that namespace.
