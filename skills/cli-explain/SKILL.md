---
name: cli-explain
description: Look up what a diagnostic code means with `rk explain`. Use whenever an E-code or L-code appears and its cause or fix is not obvious.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Explains any code the compiler or the linter can emit.
What it means, why it fires, and an example that produces it.

## Usage

`[ERROR_CODE]` The code to look up.
`E` for the compiler, `L` for the linter.
