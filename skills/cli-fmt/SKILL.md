---
name: cli-fmt
description: Format every .st file in a workspace with `rk fmt`. Use when asked to format, tidy or normalise source, or to check formatting before a commit.
---

## Summary

`fmt` will analyze and try to format all files in a workspace, it will abort on files that have invalid syntax.
Note that compilation errors are not considered syntax errors.

## Usage

`--check` Checks and lists if some files needs formatting without writing anything.

`--workspace <WORKSPACE>` Sets the workspace path to format, by default .

`--output-format <OUTPUT_FORMAT>` Possible values are full, concise, json-lines.