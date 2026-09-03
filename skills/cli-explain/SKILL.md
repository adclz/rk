---
name: cli-explain
description: Look up what a diagnostic code means with `rk explain`. Use whenever an E-code or L-code appears and its cause or fix is not obvious.
---

## Summary

Explain can show the informations about any code shown by the compiler or the linter.

## Usage

`[ERROR_CODE]` The error code you want to know more about, starts with either E or L. 

`--output-format <OUTPUT_FORMAT>` Possible values are full, concise, json-lines.