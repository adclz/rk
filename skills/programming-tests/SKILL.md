---
name: programming-tests
description: Write unit tests in Structured Text — the {test} pragma, Std.Unit's
  ASSERT/ASSERT_EQ/ASSERT_NEQ, and how to drive a stateful FUNCTION_BLOCK from a
  test. Use when adding or fixing tests for ST code. Running them is `cli-test`.
---

## Summary

A test is a `FUNCTION` marked `{test}`.
It takes no arguments and returns nothing useful; it asserts, and an assertion that fails raises, which is what marks the test failed.

```iecst
USING Std.Unit;

{test}
FUNCTION test_addition_wraps_at_16_bits
	VAR x : INT := 32767; END_VAR
	x := x + 1;
	ASSERT_EQ(value := x, target := INT#-32768, message := 'INT wraps');
END_FUNCTION
```

`{test}` is valid ONLY on a `FUNCTION`, like `{extern}`: on a `FUNCTION_BLOCK`, a `PROGRAM` or a `METHOD` it is `E1503`.
A test is a `()` entry the runner calls, and no other POU kind has one; a POU that exists only for tests is not a test — hide it with `FUNCTION PRIVATE` instead.

A test may declare a return type and inputs; nothing supplies them, so do not.
Two `{test}` functions may not share a name (E0102) — but a name is qualified by its namespaces, so `Deep.Nest.test_x` and a top-level `test_x` coexist, and that path is what `rk test <name>` matches.

## Assertions

From `Std.Unit`, so `USING Std.Unit;` is required:

- `ASSERT(value := <BOOL>, message := '…')` — fails when the condition is FALSE.
- `ASSERT_EQ(value := …, target := …, message := '…')` — fails when they differ.
- `ASSERT_NEQ(…)` — the inverse.

`ASSERT_EQ`/`ASSERT_NEQ` are overload sets covering every elementary type, STRING and CHAR included; the two arguments must land on ONE overload, so compare like with like (an `INT` against a `DINT` is fine — it widens — but prefer typed literals: `INT#1`, not `1`).
A call no overload accepts is E0810, which lists what each overload takes.

`message` defaults to empty, and the failure report names the file and line, so a message is only worth writing when the line alone will not say WHICH assertion of several failed.
Write what the code should have done, not "failed":

```iecst fragment
ASSERT_EQ(value := c.CV, target := INT#1, message := 'one rising edge counted once');
```

## Testing a stateful FUNCTION_BLOCK

An FB keeps state between calls, so a test drives it the way a scan would — call it repeatedly and assert between calls.
This is how edge behaviour, latching and instance independence get pinned:

```iecst
USING Std.Unit;

FUNCTION_BLOCK Counter
	VAR_INPUT CU : BOOL; END_VAR
	VAR_OUTPUT CV : INT; END_VAR
	VAR prev : BOOL; END_VAR
	IF CU AND NOT prev THEN
		CV := CV + 1;
	END_IF;
	prev := CU;
END_FUNCTION_BLOCK

{test}
FUNCTION test_edge_counting
	VAR c : Counter; END_VAR
	c(CU := TRUE);
	ASSERT_EQ(value := c.CV, target := INT#1, message := 'one edge');
	c(CU := TRUE);
	ASSERT_EQ(value := c.CV, target := INT#1, message := 'level, not edge');
	c(CU := FALSE);
	c(CU := TRUE);
	ASSERT_EQ(value := c.CV, target := INT#2, message := 'second edge');
END_FUNCTION
```

Each test gets fresh instances — a `VAR c : Counter;` starts at its initializers every run — so tests do not leak state into each other, and two instances in one test are independent.

## Waiting for something asynchronous

Nothing blocks: a block backed by a host driver completes across *calls*, not inside one.
Spin it on a deadline rather than a fixed count, so a slow machine does not fail the test and a broken driver does not hang it:

```iecst sketch
USING Std.Timers;

FUNCTION PRIVATE drive_until_done
	VAR_IN_OUT
		drv : Driver;      // a block that stays BUSY until its host answers
	END_VAR
	VAR start : TIME; END_VAR
	start := PLC_TIME();
	WHILE drv.BUSY AND ((PLC_TIME() - start) < T#3s) DO
		drv(REQ := TRUE);
	END_WHILE;
END_FUNCTION
```

`rk test --timeout` is the backstop for a spin that never settles; the in-test deadline is what turns a hang into a readable assertion failure.

## Organising

Put tests in a nested `Test` namespace beside the code they cover — the standard library's own convention:

```iecst sketch
NAMESPACE Std.Counters
	FUNCTION_BLOCK CTU … END_FUNCTION_BLOCK

	NAMESPACE Test
		USING Std.Unit;
		USING Std.Counters;

		{test}
		FUNCTION test_counts_up … END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
```

That keeps test names out of a consumer's unqualified scope and stops them colliding with library names, while `rk test` still finds them.

## Shared setup

A test is one FUNCTION that builds what it needs.
Shared setup is an ordinary `PRIVATE` helper function the tests call — like `drive_until_done` above — which the compiler keeps inside its namespace (E1005), so a helper never leaks into the library's API.
