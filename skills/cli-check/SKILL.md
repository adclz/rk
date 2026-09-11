---
name: cli-check
description: Check a workspace for diagnostics with `rk check` without producing a binary. Use when asked whether code compiles, to see all errors and warnings, or before committing .st changes.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Runs the static analyzer over the workspace.
Nothing is compiled and nothing is written.

Reports every compilation error, plus the linter's warnings, hints and info.

## Usage

`-w, --watch` Re-run on every file change.

`--workspace <WORKSPACE>` Workspace path, `.` by default.
