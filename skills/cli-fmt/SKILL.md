---
name: cli-fmt
description: Format every .st file in a workspace with `rk fmt`. Use when asked to format, tidy or normalise source, or to check formatting before a commit.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Formats every `.st` file in the workspace.

A file that does not parse is refused and left untouched.
Compilation errors are not syntax errors: a file that fails to type-check still formats.

The `;` at the end of a declaration, a statement or a `USING` is optional to the
parser, and the formatter writes it in. One already there is left alone, and a
`USING` naming several namespaces takes one terminator at the end.

## Usage

`--check` Report what would change; write nothing.
Exits 1 if anything would.

`--workspace <WORKSPACE>` Workspace path, `.` by default.
