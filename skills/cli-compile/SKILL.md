---
name: cli-compile
description: Compile a workspace to a WebAssembly module with `rk compile` — the debug and release profiles, the optimization levels, and where wasm-opt comes from. Use when asked to build, to produce a release binary, or when an optimized build fails.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Compiles the workspace to one core WebAssembly module.

A workspace with errors does not compile, and `rk check` reports the same diagnostics without writing anything.
The linter does not run here, so a lint can never block a build.

The module is written to `rk_build/debug/core.wasm`, or to `rk_build/release/core.wasm` with `--release`.

## Usage

`--release` The release profile: optimized by wasm-opt, stepping tables omitted.

`-O, --opt-level <OPT_LEVEL>` The optimization level of a release build: `0` to `4`, `s` (size) or `z` (aggressive size).
Defaults to `2`, or to `opt_level` in `config.toml`; the flag wins.

`-o, --output <OUTPUT>` Where to write the module.

`-w, --watch` Re-compile on every file change.

`--workspace <WORKSPACE>` Workspace path, `.` by default.

`-v, --verbose` Verbose output.

## Profiles

There are two profiles, debug and release.

| | `rk compile` | `rk compile --release` |
| --- | --- | --- |
| stepping tables | yes | no |
| symbols, retain map, schedule | yes | yes |
| optimized by wasm-opt | never | always |

The stepping tables point inside the code: `debug-lines` maps a code offset to a line, `debug-functions` a WASM function index to a POU, `debug-locals` a WASM local slot to a variable.
A release build is optimized by Binaryen, which rewrites the body of every function, so those tables would point to the wrong place.
This is why a release build does not carry them: it is watchable, but not steppable.

The memory layout is identical between the two profiles.
That is what lets a host stop a release build, rebuild the same source as debug, and carry the live state across.
Variables can still be read and written by name in a release build, because `debug-symbols` points to the memory and not to the code.

Most of what a release build saves comes from the dropped tables, not from the optimized code.

## wasm-opt

`wasm-opt` is an external binary.
If one is on `PATH`, it is used.
Otherwise Binaryen 131 is downloaded once into the user cache directory (`~/.cache/rk/binaryen` on Linux), and its checksum is verified.
Setting `RK_NO_DOWNLOAD` to any value opts out of the download.

`-O4` runs with `--skip-pass=flatten`: the Flatten pass of Binaryen does not support `try_table` yet (WebAssembly/binaryen#8372).
What is left of `-O4` is close to `-O3`.

If the optimizer fails, `rk compile --release` refuses outright, where `rk test -O` warns and runs a correct unoptimized build.
A release build never silently degrades.

## Reference files

Read the one that covers the question; each is self-contained.

- `references/abi.md` — what the module imports and exports, how each ST type crosses a call boundary, and how to call an export
- `references/sections.md` — the custom sections: symbols, schedule, retain map, test manifest, and the exception a module raises
