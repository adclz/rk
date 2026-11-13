# Tests

Mark a `FUNCTION` with `{test}` and it becomes a test.
It checks its results with the assertions of `Std.Unit`:

```st
USING Std.Unit;

{test}
FUNCTION test_add
    ASSERT_EQ(Add(a := 2, b := 3), INT#5, 'Add(2, 3)');
END_FUNCTION
```

`rk test` compiles the workspace and runs all of them, showing a summary at the end.
It runs them on the component of the workspace, unoptimized, the way a host runs it, and leaves it in `rk_build/test/` with its WIT.
That build tells `rk test` where each raise is (below), so no other command writes it.
The clocks are WASI's, as wasmtime gives them, and the time zone is the machine's.
It calls a test with no arguments, so a test FUNCTION has no `VAR_INPUT`, `VAR_OUTPUT` or `VAR_IN_OUT` (`E1511`).

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

## A test that runs over cycles

A test FUNCTION runs once, in one call.
A `FUNCTION_BLOCK` marked `{test}` runs once per cycle, on an instance of its own, until a cycle leaves its `DONE` output TRUE: the test has then passed.
Its variables keep their values from one cycle to the next, as a block's do in a task, and it can hold what a FUNCTION cannot, a timer or a `STDOUT`.

```st
USING Std.Timers;

FUNCTION_BLOCK Valve
	VAR_INPUT
		open: BOOL;
	END_VAR
	VAR_OUTPUT
		opened: BOOL;
	END_VAR
	VAR
		travel: TON;
	END_VAR
	travel(IN := open, PT := T#50ms);
	opened := travel.Q;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_the_valve_opens
	VAR_OUTPUT
		DONE: BOOL;
	END_VAR
	VAR
		inlet: Valve;
	END_VAR
	inlet(open := TRUE);
	DONE := inlet.opened;
END_FUNCTION_BLOCK
```

`rk test` runs the cycles one after the other, with no wait between them: this test passes after 50 ms.
The block declares its `DONE`, a `BOOL` in `VAR_OUTPUT` (`E1517`), and takes no `VAR_INPUT` or `VAR_IN_OUT` (`E1511`).
A raise fails the test in the cycle that raised.
A block still running when its time is spent fails too, and the report says how many cycles it ran, here with a valve that takes 10 s to open:

```sh
        FAIL test_the_valve_opens (main.st:18): the watchdog stopped it after 5s: DONE was still FALSE after 1147027 cycles
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
A test is given 5 seconds, which `--timeout` changes: each of its calls has that time to return, and a test block that time to set its `DONE`.
A trap comes back with the calls in progress.
An exception does not, so the module `rk test` builds calls `rk test` just before each raise, through `rk:test/raise`, and the calls are taken there.
An exception that went through a `__FINALLY` keeps its place, and one raised again in a `__CATCH` is placed at that `__RAISE`.

`rk test -O` runs an optimized build, which has lost its line tables: a failure there shows its message and the line of its test, without the calls.

With `--output-format concise` the calls follow the reason on the same line, and with `json-lines` they are the record's `backtrace`.

What a test writes to the standard output and error with [`Std.Console`](console.md) is shown under its calls, both outputs in the order written.
A test block holds the blocks it writes with:

```st
USING Std.Console;
USING Std.Unit;

{test}
FUNCTION_BLOCK test_pressure
	VAR_OUTPUT
		DONE: BOOL;
	END_VAR
	VAR
		bar: REAL := 3.2;
		out: STDOUT;
		err: STDERR;
	END_VAR
	out(REQ := TRUE, IN := 'reading the sensor$N');
	err(REQ := TRUE, IN := 'pressure is high$N');
	ASSERT(bar < 3.0, 'pressure under 3 bar');
	DONE := TRUE;
END_FUNCTION_BLOCK
```

```sh
     Failures:
        FAIL test_pressure (main.st:5): assertion failed: pressure under 3 bar
             at Std.Unit.ASSERT (<lib>/Unit.st:25:4)
             at test_pressure (main.st:16:2)
             output:
               reading the sensor
               pressure is high
```

A test FUNCTION holds no block that writes (`E1515`): it writes through blocks declared in a `VAR_GLOBAL`, which the host takes within the call.

What a passing test writes is not shown.
A test keeps the first 64 KiB it writes, and a last line says how many bytes came after.
In the concise format the output follows the calls, in quotes with its line breaks written `\n`, and in `json-lines` it is the record's `output`.

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

- Internally all assertions use the built-in `__RAISE` that triggers a WASM exception with a STRING payload, see [Exceptions](exceptions.md)
  
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
