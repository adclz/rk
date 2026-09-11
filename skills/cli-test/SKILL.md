---
name: cli-test
description: Compile a workspace and run its {test} functions with `rk test`. Use when asked to run tests, verify a change, or narrow a run to one test.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Runs every FUNCTION marked `{test}`.
Writing them is the `programming-tests` skill.

Exit code is `0` when all passed, `1` otherwise.
That same `1` is also a workspace that failed to compile, so read the output to tell the two apart.

```console
    FAIL [611.8ms] failing
    PASS [502.9ms] passing
────────────────────────────────────────────
 Failures:
    FAIL failing (main.st:7): assertion failed:
────────────────────────────────────────────
 Summary [  1.18s] 2 tests run: 1 passed, 1 failed
```

Assertions are `Std.Unit`'s `ASSERT`, `ASSERT_EQ` and `ASSERT_NEQ`, taking `value` and `target`.
They raise, and that is what fails a test.
They need the standard library: if `rk env stdlib` prints nothing, the run stops at `E0201` before any test executes.

## Usage

`[TEST_NAME]` Run tests whose name contains this.
A namespaced test is matched by its path, `Ns1.Ns2.TestName`.

`-O, --opt-level <OPT_LEVEL>` 0-4, `s`, `z`.
Tests default to the unoptimized core; this checks an optimized build computes the same thing.

`--timeout <TIMEOUT>` How long one test may run, e.g. `30s`, `500ms`.
A test waiting out a timer legitimately runs long; one that never returns must not hang the run.

`--workspace <WORKSPACE>` Workspace path, `.` by default.
