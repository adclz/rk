---
name: programming-tests
description: Write unit tests in Structured Text — the {test} pragma, Std.Unit's
  ASSERT/ASSERT_EQ/ASSERT_NEQ, how to drive a stateful FUNCTION_BLOCK from a
  test, and a test FUNCTION_BLOCK that runs over cycles. Use when adding or
  fixing tests for ST code. Running them is `cli-test`.
---

## Summary

A test is a `FUNCTION` marked `{test}`, which runs once, or a `FUNCTION_BLOCK` marked `{test}`, which runs cycle by cycle (below).
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

`{test}` is valid ONLY on a `FUNCTION` and on a `FUNCTION_BLOCK` the runner can make an instance of: on a `PROGRAM`, a `METHOD`, or an `ABSTRACT` or `{extern}` FUNCTION_BLOCK it is `E1503`.
A POU that exists only for tests is not a test — hide it with `FUNCTION PRIVATE` instead.

A test FUNCTION may declare a return type, which nothing reads; it takes no `VAR_INPUT`, `VAR_OUTPUT` or `VAR_IN_OUT` (E1511), since nothing supplies them.
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

What a test writes with `Std.Console`'s `STDOUT` or `STDERR` blocks is shown under its failure, both outputs in the order written (the first 64 KiB), and dropped when it passes, so a value written before an assertion helps read why it failed.
A test FUNCTION_BLOCK holds the blocks itself; a test FUNCTION holds none (E1515), and writes through blocks declared in a `VAR_GLOBAL`.

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

## A test over cycles

Nothing blocks: a timer, or a block backed by the host, completes across *cycles*, not inside one call, and a loop inside a test FUNCTION never sees the host answer.
A `FUNCTION_BLOCK` marked `{test}` runs once per cycle, on an instance of its own, until a cycle leaves its `DONE` output TRUE: the test has then passed.
Its variables keep their values from one cycle to the next, so a sequence is a step variable advanced as the cycles go:

```iecst
USING Std.Timers;
USING Std.Unit;

FUNCTION_BLOCK Debounce
	VAR_INPUT raw : BOOL; END_VAR
	VAR_OUTPUT stable : BOOL; END_VAR
	VAR hold : TON; END_VAR
	hold(IN := raw, PT := T#20ms);
	stable := hold.Q;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_debounce_waits_its_time
	VAR_OUTPUT DONE : BOOL; END_VAR
	VAR
		button : Debounce;
		first : BOOL := TRUE;
	END_VAR
	button(raw := TRUE);
	IF first THEN
		ASSERT(value := NOT button.stable, message := 'stable at once');
		first := FALSE;
	END_IF;
	DONE := button.stable;
END_FUNCTION_BLOCK
```

The block declares `DONE`, a `BOOL` in `VAR_OUTPUT` (E1517), and takes no `VAR_INPUT` or `VAR_IN_OUT` (E1511); other outputs are its own.
The cycles run one after the other, with no wait between them.
A raise fails the test in the cycle that raised.
A block still running when `rk test --timeout` (5 s by default) is spent fails with the number of cycles it ran, so a test that waits on the host needs no deadline of its own.
Nothing may name a test block (E1007): shared setup goes in an ordinary block the test holds.

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

A test is one FUNCTION or FUNCTION_BLOCK that builds what it needs.
Shared setup is an ordinary `PRIVATE` helper function the tests call, which the compiler keeps inside its namespace (E1005), so a helper never leaks into the library's API.
