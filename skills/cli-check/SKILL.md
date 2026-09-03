---
name: cli-check
description: Check a workspace for diagnostics with `rk check` without producing a binary. Use when asked whether code compiles, to see all errors and warnings, or before committing .st changes.
---

## Summary

Running `check` triggers the static analyzer to check the current workspace, note that this does not mean it is compiling anything.

`check` will return a list of all compilation errors if any, and all the informations / warning / hints from the linter.

## Usage

`-w, --watch` Continuously watch the workspace on any file change and rerun.

`--workspace <WORKSPACE>` Sets the workspace path to check, by default .

`--output-format <OUTPUT_FORMAT>` Possible values are full, concise, json-lines.

full is readbale for humans, concise will show one line per diagnostic, json-lines will output the diagnostics in JSON format.