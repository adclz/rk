---
name: cli-test
description: Compile a workspace and run its {test} functions with `rk test`. Use when asked to run tests, verify a change, or narrow a run to one test.
---

## Summary

`test` executes all the FUNCTION POUs marked with `{test}`. WRITING those
tests — the pragma's rules, `Std.Unit`'s assertions, driving a stateful FB —
is the `programming-tests` skill.

The exit code is `0` when every test passed and `1` otherwise. It is the same `1` for a workspace that failed to compile, so an agent must read the output to tell a build failure from a test failure, not the code alone.

Each test is reported as it finishes, `PASS` or `FAIL` with its duration. Failures are then repeated in a `Failures:` block with the assertion site, and a summary of all the tests passed and failed is shown at the end of execution.

```
    FAIL [611.8ms] failing
    PASS [502.9ms] passing
────────────────────────────────────────────
 Failures:
    FAIL failing (main.st:7): assertion failed:
────────────────────────────────────────────
 Summary [  1.18s] 2 tests run: 1 passed, 1 failed
```

Assertions come from the standard library, `Std.Unit.ASSERT`, `ASSERT_EQ` and `ASSERT_NEQ`. `ASSERT_EQ` takes `value` and `target`. They raise on failure, which is what marks the test failed. Reaching them requires a standard library, which resolves on its own from the `rk` binary's own location. If `rk env stdlib` prints nothing, `Std.*` will not resolve and the run stops on `E0204` before any test executes.

## Usage

`[TEST_NAME]` Run a specific test by name (substring match).
Note that a test name is influenced by their location in the codebase.

- If a test is not inside a namespace, it can be called directly by its name.
- If a test is inside or or multiple namespaces, the path could be `Ns1.Ns2.TestName`.  

`-O, --opt-level <OPT_LEVEL>` Optimization level: 0-4, s (size), z (aggressive size). Tests default to the unoptimized core; this checks that an optimized build still computes the same thing.

`--timeout <TIMEOUT>` How long one test may run before it is stopped, e.g. `30s`, `500ms`. A test that waits out a timer's preset legitimately runs long; a test that never returns must still not hang the run.

`--workspace <WORKSPACE>` Sets the workspace path to test, by default .

`--output-format <OUTPUT_FORMAT>` Possible values are full, concise, json-lines.

full is readable for humans, concise will show one line per diagnostic, json-lines will output the diagnostics in JSON format.